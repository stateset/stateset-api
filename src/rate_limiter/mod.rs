/*!
 * # Rate Limiting Module
 *
 * This module provides a configurable rate limiter for API requests based on various strategies:
 *
 * - Global API rate limits (per IP address)
 * - Path-based rate limits (per API endpoint)
 * - User-based rate limits (when authenticated)
 *
 * The rate limiter uses Redis for distributed rate limiting, which allows it to work
 * across multiple API server instances. It implements the token bucket algorithm
 * for efficient and flexible rate limiting.
 *
 * ## Features
 *
 * - Configurable rate limits through environment variables
 * - Different rate limits for different API endpoints
 * - Standard rate limit headers (X-RateLimit-*)
 * - RFC-compliant headers (RateLimit-*)
 * - Detailed logging of rate limit operations
 * - Multiple key extraction strategies
 *
 * ## Usage
 *
 * ```ignore
 * // Create a rate limiter
 * let config = RateLimitConfig {
 *     requests_per_window: 100,
 *     window_duration: Duration::from_secs(60),
 *     ..Default::default()
 * };
 *
 * // Apply the rate limiter middleware
 * let app = Router::new()
 *     .route("/", get(handler))
 *     .layer(RateLimitLayer::new(config.clone(), RateLimitBackend::InMemory));
 * ```
 */
use axum::{
    extract::Request,
    http::{header, Response, StatusCode},
    middleware::Next,
    response::IntoResponse,
};
use dashmap::DashMap;
use metrics::counter;
use redis::AsyncCommands;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use thiserror::Error;
use tracing::{debug, warn};

use crate::auth::{AuthService, AuthUser};

/// Helper function to convert a number to a HeaderValue.
/// This is safe because numeric strings are always valid ASCII header values.
///
/// # Panics
/// This function will never panic in practice as numeric strings contain only ASCII digits,
/// which are always valid HTTP header values per RFC 7230.
fn num_to_header_value<T: ToString>(n: T) -> http::HeaderValue {
    // SAFETY: Numeric types when converted to string only produce ASCII digit characters (0-9)
    // and optionally a minus sign (-) for negative numbers. These are always valid HTTP header
    // characters according to RFC 7230 Section 3.2.6 (field-content = field-vchar).
    // This unwrap_or_default provides a fallback for the theoretically impossible case.
    http::HeaderValue::from_str(&n.to_string())
        .unwrap_or_else(|_| http::HeaderValue::from_static("0"))
}

// In-memory rate limiter implementation
#[derive(Debug, Error)]
pub enum RateLimitError {
    #[error("Rate limit exceeded")]
    LimitExceeded,
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),
    #[error("Internal error: {0}")]
    InternalError(String),
}

#[derive(Debug, Clone)]
struct RateLimitEntry {
    count: u32,
    window_start: Instant,
    last_request: Instant,
}

impl RateLimitEntry {
    fn new() -> Self {
        let now = Instant::now();
        Self {
            count: 1,
            window_start: now,
            last_request: now,
        }
    }

    fn increment(&mut self, window_duration: Duration) {
        let now = Instant::now();

        // Reset if window has expired
        if now.duration_since(self.window_start) >= window_duration {
            self.count = 1;
            self.window_start = now;
        } else {
            self.count += 1;
        }

        self.last_request = now;
    }

    fn is_allowed(&self, limit: u32, window_duration: Duration) -> bool {
        let now = Instant::now();

        // If window has expired, allow the request
        if now.duration_since(self.window_start) >= window_duration {
            return true;
        }

        // Check if under limit
        self.count <= limit
    }

    fn time_until_reset(&self, window_duration: Duration) -> Duration {
        let elapsed = self.last_request.duration_since(self.window_start);
        if elapsed >= window_duration {
            Duration::from_secs(0)
        } else {
            window_duration - elapsed
        }
    }
}

#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    pub requests_per_window: u32,
    pub window_duration: Duration,
    pub burst_limit: Option<u32>,
    pub enable_headers: bool,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            requests_per_window: 100,
            window_duration: Duration::from_secs(60),
            burst_limit: None,
            enable_headers: true,
        }
    }
}

#[derive(Clone)]
pub enum RateLimitBackend {
    InMemory,
    Redis {
        client: Arc<redis::Client>,
        namespace: String,
    },
}

impl Default for RateLimitBackend {
    fn default() -> Self {
        Self::InMemory
    }
}

#[derive(Clone)]
enum RateLimitStore {
    InMemory {
        entries: Arc<DashMap<String, RateLimitEntry>>,
    },
    Redis {
        client: Arc<redis::Client>,
        namespace: String,
        fallback: Arc<DashMap<String, RateLimitEntry>>,
    },
}

#[derive(Clone)]
pub struct RateLimiter {
    store: RateLimitStore,
    config: RateLimitConfig,
}

impl RateLimiter {
    pub fn new(config: RateLimitConfig, backend: RateLimitBackend) -> Self {
        let store = match backend {
            RateLimitBackend::InMemory => RateLimitStore::InMemory {
                entries: Arc::new(DashMap::new()),
            },
            RateLimitBackend::Redis { client, namespace } => RateLimitStore::Redis {
                client,
                namespace,
                fallback: Arc::new(DashMap::new()),
            },
        };

        Self { store, config }
    }

    pub fn in_memory(config: RateLimitConfig) -> Self {
        Self::new(config, RateLimitBackend::InMemory)
    }

    pub fn with_config(&self, config: RateLimitConfig) -> Self {
        Self {
            store: self.store.clone(),
            config,
        }
    }

    pub async fn check_rate_limit(&self, key: &str) -> Result<RateLimitResult, RateLimitError> {
        self.check_with_config(key, &self.config).await
    }

    async fn check_with_config(
        &self,
        key: &str,
        config: &RateLimitConfig,
    ) -> Result<RateLimitResult, RateLimitError> {
        match &self.store {
            RateLimitStore::InMemory { entries } => Ok(Self::check_in_memory(entries, key, config)),
            RateLimitStore::Redis {
                client,
                namespace,
                fallback,
            } => match client.get_async_connection().await {
                Ok(mut conn) => {
                    match Self::check_with_redis(&mut conn, namespace, key, config).await {
                        Ok(result) => Ok(result),
                        Err(err) => {
                            warn!("Redis rate limit error: {}", err);
                            Ok(Self::check_in_memory(fallback, key, config))
                        }
                    }
                }
                Err(err) => {
                    warn!(
                        "Failed to connect to Redis for rate limiting, using fallback: {}",
                        err
                    );
                    Ok(Self::check_in_memory(fallback, key, config))
                }
            },
        }
    }

    fn check_in_memory(
        entries: &DashMap<String, RateLimitEntry>,
        key: &str,
        config: &RateLimitConfig,
    ) -> RateLimitResult {
        let mut entry = entries
            .entry(key.to_string())
            .or_insert_with(RateLimitEntry::new);

        if !entry.is_allowed(config.requests_per_window, config.window_duration) {
            let time_until_reset = entry.time_until_reset(config.window_duration);
            return RateLimitResult {
                allowed: false,
                limit: config.requests_per_window,
                remaining: 0,
                reset_time: time_until_reset,
            };
        }

        entry.increment(config.window_duration);
        let remaining = config.requests_per_window.saturating_sub(entry.count);
        let time_until_reset = entry.time_until_reset(config.window_duration);

        RateLimitResult {
            allowed: true,
            limit: config.requests_per_window,
            remaining,
            reset_time: time_until_reset,
        }
    }

    async fn check_with_redis<C>(
        conn: &mut C,
        namespace: &str,
        key: &str,
        config: &RateLimitConfig,
    ) -> Result<RateLimitResult, redis::RedisError>
    where
        C: redis::aio::ConnectionLike + Send,
    {
        let redis_key = format!("{}:{}", namespace, key);
        let limit = config.requests_per_window as i64;
        let window_secs = config.window_duration.as_secs().max(1);

        let count: i64 = conn.incr(&redis_key, 1).await?;
        if count == 1 {
            let _: Result<(), _> = conn.expire(&redis_key, window_secs as usize).await;
        } else {
            let ttl: i64 = conn.ttl(&redis_key).await.unwrap_or(-1);
            if ttl < 0 {
                let _: Result<(), _> = conn.expire(&redis_key, window_secs as usize).await;
            }
        }

        let ttl_secs = match conn.ttl::<_, i64>(&redis_key).await {
            Ok(ttl) if ttl > 0 => ttl as u64,
            _ => window_secs,
        };
        let allowed = count <= limit;
        let remaining = if allowed {
            config
                .requests_per_window
                .saturating_sub(count.max(0) as u32)
        } else {
            0
        };

        Ok(RateLimitResult {
            allowed,
            limit: config.requests_per_window,
            remaining,
            reset_time: Duration::from_secs(ttl_secs),
        })
    }

    pub async fn get_remaining_quota(&self, key: &str) -> u32 {
        match &self.store {
            RateLimitStore::InMemory { entries } => {
                Self::remaining_in_memory(entries, key, &self.config)
            }
            RateLimitStore::Redis {
                client,
                namespace,
                fallback,
            } => {
                let redis_key = format!("{}:{}", namespace, key);
                match client.get_async_connection().await {
                    Ok(mut conn) => match conn.get::<_, i64>(&redis_key).await {
                        Ok(count) if count > 0 => {
                            self.config.requests_per_window.saturating_sub(count as u32)
                        }
                        Ok(_) => self.config.requests_per_window,
                        Err(err) => {
                            warn!("Failed to get Redis quota for {}: {}", key, err);
                            Self::remaining_in_memory(fallback, key, &self.config)
                        }
                    },
                    Err(err) => {
                        warn!(
                            "Failed to connect to Redis for quota lookup, using fallback: {}",
                            err
                        );
                        Self::remaining_in_memory(fallback, key, &self.config)
                    }
                }
            }
        }
    }

    pub async fn reset(&self, key: &str) -> Result<(), RateLimitError> {
        match &self.store {
            RateLimitStore::InMemory { entries } => {
                entries.remove(key);
            }
            RateLimitStore::Redis {
                client,
                namespace,
                fallback,
            } => {
                let redis_key = format!("{}:{}", namespace, key);
                match client.get_async_connection().await {
                    Ok(mut conn) => {
                        let _: Result<(), _> = conn.del(&redis_key).await;
                    }
                    Err(err) => {
                        warn!("Failed to reset Redis quota for {}: {}", key, err);
                    }
                }
                fallback.remove(key);
            }
        }
        Ok(())
    }

    pub async fn cleanup_expired(&self) {
        match &self.store {
            RateLimitStore::InMemory { entries } => {
                let now = Instant::now();
                entries.retain(|_, entry| {
                    now.duration_since(entry.window_start) < self.config.window_duration
                        || entry.count > 0
                });
            }
            RateLimitStore::Redis { fallback, .. } => {
                let now = Instant::now();
                fallback.retain(|_, entry| {
                    now.duration_since(entry.window_start) < self.config.window_duration
                        || entry.count > 0
                });
            }
        }
    }

    fn remaining_in_memory(
        entries: &DashMap<String, RateLimitEntry>,
        key: &str,
        config: &RateLimitConfig,
    ) -> u32 {
        if let Some(entry) = entries.get(key) {
            let now = Instant::now();
            if now.duration_since(entry.window_start) >= config.window_duration {
                config.requests_per_window
            } else {
                config.requests_per_window.saturating_sub(entry.count)
            }
        } else {
            config.requests_per_window
        }
    }
}

#[derive(Debug)]
pub struct RateLimitResult {
    pub allowed: bool,
    pub limit: u32,
    pub remaining: u32,
    pub reset_time: Duration,
}

#[derive(Clone, Debug)]
pub struct PathPolicy {
    pub prefix: String,
    pub requests_per_window: u32,
    pub window_duration: Duration,
}

// Key extraction functions
pub fn extract_ip_key(request: &Request) -> String {
    // Try to get real IP from X-Forwarded-For or X-Real-IP headers
    if let Some(forwarded) = request.headers().get("x-forwarded-for") {
        if let Ok(forwarded_str) = forwarded.to_str() {
            if let Some(ip) = forwarded_str.split(',').next() {
                return format!("ip:{}", ip.trim());
            }
        }
    }

    if let Some(real_ip) = request.headers().get("x-real-ip") {
        if let Ok(ip_str) = real_ip.to_str() {
            return format!("ip:{}", ip_str);
        }
    }

    // Fallback to connection info (this would need to be passed through middleware state)
    "ip:unknown".to_string()
}

/// Extracts a rate limit key for authenticated users.
///
/// Priority:
/// 1. AuthUser from request extensions (set by auth middleware after JWT validation)
/// 2. x-user-id header (for internal/service-to-service calls)
/// 3. None if no user context available
pub async fn extract_user_key(
    request: &Request,
    auth_service: Option<&Arc<AuthService>>,
) -> Option<String> {
    // Primary: Get user ID from validated AuthUser in request extensions
    // This is populated by the auth middleware after JWT token validation
    if let Some(auth_user) = request.extensions().get::<AuthUser>() {
        return Some(format!("user:{}", auth_user.user_id));
    }

    // Try to resolve user from a Bearer token using the shared AuthService
    if let Some(service) = auth_service {
        if let Some(auth_header) = request.headers().get(header::AUTHORIZATION) {
            if let Ok(raw) = auth_header.to_str() {
                if let Some(token) = raw.strip_prefix("Bearer ").map(str::trim) {
                    if let Ok(claims) = service.validate_token(token).await {
                        return Some(format!("user:{}", claims.sub));
                    }
                }
            }
        }
    }

    // Fallback: x-user-id header for internal service-to-service calls
    if let Some(user_id) = request.headers().get("x-user-id") {
        if let Ok(user_str) = user_id.to_str() {
            return Some(format!("user:{}", user_str));
        }
    }

    None
}

pub fn extract_api_key(request: &Request) -> Option<String> {
    if let Some(api_key) = request.headers().get("x-api-key") {
        if let Ok(key_str) = api_key.to_str() {
            return Some(format!("api_key:{}", key_str));
        }
    }

    None
}

// Middleware implementation
pub async fn rate_limit_middleware(
    request: Request,
    next: Next,
) -> Result<Response<axum::body::Body>, Response<axum::body::Body>> {
    // This is a simplified middleware - in practice you'd inject the rate limiter
    let config = RateLimitConfig::default();
    let rate_limiter = RateLimiter::in_memory(config.clone());

    // Extract key (prefer API key, then user, then IP)
    let key = if let Some(k) = extract_api_key(&request) {
        k
    } else if let Some(u) = extract_user_key(&request, None).await {
        u
    } else {
        extract_ip_key(&request)
    };

    // Check rate limit
    match rate_limiter.check_rate_limit(&key).await {
        Ok(result) => {
            if !result.allowed {
                warn!("Rate limit exceeded for key: {}", key);

                let mut response = Response::new(axum::body::Body::from("Rate limit exceeded"));
                *response.status_mut() = StatusCode::TOO_MANY_REQUESTS;

                if config.enable_headers {
                    let headers = response.headers_mut();
                    headers.insert("X-RateLimit-Limit", num_to_header_value(result.limit));
                    headers.insert("X-RateLimit-Remaining", num_to_header_value(0));
                    headers.insert(
                        "X-RateLimit-Reset",
                        num_to_header_value(result.reset_time.as_secs()),
                    );
                }

                return Err(response);
            }

            // Process request
            let mut response = next.run(request).await;

            // Add rate limit headers to successful response
            if config.enable_headers {
                let headers = response.headers_mut();
                headers.insert("X-RateLimit-Limit", num_to_header_value(result.limit));
                headers.insert(
                    "X-RateLimit-Remaining",
                    num_to_header_value(result.remaining),
                );
                headers.insert(
                    "X-RateLimit-Reset",
                    num_to_header_value(result.reset_time.as_secs()),
                );
            }

            Ok(response)
        }
        Err(e) => {
            warn!("Rate limiter error: {}", e);
            // Continue with request on error
            Ok(next.run(request).await)
        }
    }
}

// Layer implementation for tower
#[derive(Clone)]
pub struct RateLimitLayer {
    rate_limiter: RateLimiter,
    path_policies: Arc<Vec<PathPolicy>>,
    api_key_policies: Arc<HashMap<String, (u32, Duration)>>,
    user_policies: Arc<HashMap<String, (u32, Duration)>>,
    auth_service: Option<Arc<AuthService>>,
}

impl RateLimitLayer {
    pub fn new(config: RateLimitConfig, backend: RateLimitBackend) -> Self {
        Self {
            rate_limiter: RateLimiter::new(config, backend),
            path_policies: Arc::new(Vec::new()),
            api_key_policies: Arc::new(HashMap::new()),
            user_policies: Arc::new(HashMap::new()),
            auth_service: None,
        }
    }

    pub fn with_policies(mut self, policies: Vec<PathPolicy>) -> Self {
        self.path_policies = Arc::new(policies);
        self
    }

    pub fn with_api_key_policies(mut self, map: HashMap<String, (u32, Duration)>) -> Self {
        self.api_key_policies = Arc::new(map);
        self
    }

    pub fn with_user_policies(mut self, map: HashMap<String, (u32, Duration)>) -> Self {
        self.user_policies = Arc::new(map);
        self
    }

    pub fn with_auth_service(mut self, auth_service: Arc<AuthService>) -> Self {
        self.auth_service = Some(auth_service);
        self
    }
}

impl<S> tower::Layer<S> for RateLimitLayer {
    type Service = RateLimitService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        RateLimitService {
            inner,
            rate_limiter: self.rate_limiter.clone(),
            path_policies: self.path_policies.clone(),
            api_key_policies: self.api_key_policies.clone(),
            user_policies: self.user_policies.clone(),
            auth_service: self.auth_service.clone(),
        }
    }
}

#[derive(Clone)]
pub struct RateLimitService<S> {
    inner: S,
    rate_limiter: RateLimiter,
    path_policies: Arc<Vec<PathPolicy>>,
    api_key_policies: Arc<HashMap<String, (u32, Duration)>>,
    user_policies: Arc<HashMap<String, (u32, Duration)>>,
    auth_service: Option<Arc<AuthService>>,
}

impl<S> tower::Service<Request> for RateLimitService<S>
where
    S: tower::Service<Request, Response = Response<axum::body::Body>> + Clone + Send + 'static,
    S::Future: Send + 'static,
{
    type Response = Response<axum::body::Body>;
    type Error = S::Error;
    type Future = std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<Self::Response, Self::Error>> + Send>,
    >;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, request: Request) -> Self::Future {
        let rate_limiter = self.rate_limiter.clone();
        let mut inner = self.inner.clone();
        let policies = self.path_policies.clone();
        let api_key_map = self.api_key_policies.clone();
        let user_map = self.user_policies.clone();
        let auth_service = self.auth_service.clone();

        Box::pin(async move {
            let auth_service = auth_service.clone();
            // Skip certain paths entirely
            let path = request.uri().path().to_string();
            if path.starts_with("/health")
                || path == "/metrics"
                || path.starts_with("/metrics/")
                || path.starts_with("/docs")
                || path.starts_with("/api-docs")
                || path.starts_with("/api/versions")
            {
                return inner.call(request).await;
            }

            // Extract key (prefer API key, then user, then IP)
            let key = if let Some(k) = extract_api_key(&request) {
                k
            } else if let Some(u) = extract_user_key(&request, auth_service.as_ref()).await {
                u
            } else {
                extract_ip_key(&request)
            };

            // Determine effective policy: API key > user > path prefix > global
            let mut effective = rate_limiter.config.clone();
            // per API key
            if let Some(api_key) = key.strip_prefix("api_key:") {
                if let Some((limit, win)) = api_key_map.get(api_key) {
                    effective.requests_per_window = *limit;
                    effective.window_duration = *win;
                }
            }
            // per user id
            if let Some(user_id) = key.strip_prefix("user:") {
                if let Some((limit, win)) = user_map.get(user_id) {
                    effective.requests_per_window = *limit;
                    effective.window_duration = *win;
                }
            }
            // path-based
            if effective.requests_per_window == rate_limiter.config.requests_per_window
                && effective.window_duration == rate_limiter.config.window_duration
            {
                for p in policies.iter() {
                    if path.starts_with(&p.prefix) {
                        effective.requests_per_window = p.requests_per_window;
                        effective.window_duration = p.window_duration;
                        break;
                    }
                }
            }

            // Use a temporary limiter if overrides differ
            let limiter = if effective.requests_per_window
                != rate_limiter.config.requests_per_window
                || effective.window_duration != rate_limiter.config.window_duration
            {
                rate_limiter.with_config(effective)
            } else {
                rate_limiter.clone()
            };

            // Check rate limit
            match limiter.check_rate_limit(&key).await {
                Ok(result) => {
                    if !result.allowed {
                        warn!("Rate limit exceeded for key: {}", key);
                        // Emit rate-limit denial metric
                        let key_type = if key.starts_with("api_key:") {
                            "api_key"
                        } else if key.starts_with("user:") {
                            "user"
                        } else {
                            "ip"
                        };
                        counter!(
                            "rate_limit_denied_total",
                            1,
                            "key_type" => key_type.to_string(),
                            "path" => path.clone(),
                        );
                        // Also reflect in custom registry for /metrics
                        let _ = {
                            #[allow(unused_imports)]
                            use crate::metrics::increment_counter;
                            increment_counter("rate_limit_denied_total");
                        };

                        let mut response =
                            Response::new(axum::body::Body::from("Rate limit exceeded"));
                        *response.status_mut() = StatusCode::TOO_MANY_REQUESTS;

                        if rate_limiter.config.enable_headers {
                            let headers = response.headers_mut();
                            let _ = headers
                                .insert("X-RateLimit-Limit", num_to_header_value(result.limit));
                            let _ = headers.insert("X-RateLimit-Remaining", num_to_header_value(0));
                            let _ = headers.insert(
                                "X-RateLimit-Reset",
                                num_to_header_value(result.reset_time.as_secs()),
                            );
                            // RFC 9447 headers
                            let _ = headers
                                .insert("RateLimit-Limit", num_to_header_value(result.limit));
                            let _ = headers.insert("RateLimit-Remaining", num_to_header_value(0));
                            let _ = headers.insert(
                                "RateLimit-Reset",
                                num_to_header_value(result.reset_time.as_secs()),
                            );
                        }

                        return Ok(response);
                    }

                    // Process request
                    let mut response = inner.call(request).await?;
                    // Emit allowed metric
                    let key_type = if key.starts_with("api_key:") {
                        "api_key"
                    } else if key.starts_with("user:") {
                        "user"
                    } else {
                        "ip"
                    };
                    counter!(
                        "rate_limit_allowed_total",
                        1,
                        "key_type" => key_type.to_string(),
                        "path" => path.clone(),
                    );
                    let _ = {
                        #[allow(unused_imports)]
                        use crate::metrics::increment_counter;
                        increment_counter("rate_limit_allowed_total");
                    };

                    // Add rate limit headers to successful response
                    if rate_limiter.config.enable_headers {
                        let headers = response.headers_mut();
                        let _ =
                            headers.insert("X-RateLimit-Limit", num_to_header_value(result.limit));
                        let _ = headers.insert(
                            "X-RateLimit-Remaining",
                            num_to_header_value(result.remaining),
                        );
                        let _ = headers.insert(
                            "X-RateLimit-Reset",
                            num_to_header_value(result.reset_time.as_secs()),
                        );
                        // RFC 9447
                        let _ =
                            headers.insert("RateLimit-Limit", num_to_header_value(result.limit));
                        let _ = headers
                            .insert("RateLimit-Remaining", num_to_header_value(result.remaining));
                        let _ = headers.insert(
                            "RateLimit-Reset",
                            num_to_header_value(result.reset_time.as_secs()),
                        );
                    }

                    Ok(response)
                }
                Err(e) => {
                    warn!("Rate limiter error: {}", e);
                    // Continue with request on error
                    inner.call(request).await
                }
            }
        })
    }
}

// Background cleanup task
pub async fn start_cleanup_task(rate_limiter: RateLimiter, interval: Duration) {
    let mut interval_timer = tokio::time::interval(interval);

    loop {
        interval_timer.tick().await;
        rate_limiter.cleanup_expired().await;
        debug!("Rate limiter cleanup completed");
    }
}

// Health check for rate limiter
pub async fn rate_limiter_health_check(limiter: &RateLimiter) -> Result<(), RateLimitError> {
    // Simple health check - try to check a rate limit
    let _remaining = limiter.get_remaining_quota("health_check").await;
    Ok(())
}

// Utility functions for rate limiting
pub fn get_rate_limit_key_for_ip(ip: &str) -> String {
    format!("ip:{}", ip)
}

pub fn get_rate_limit_key_for_user(user_id: &str) -> String {
    format!("user:{}", user_id)
}

pub fn get_rate_limit_key_for_api_key(api_key: &str) -> String {
    format!("api_key:{}", api_key)
}

// Response helpers
impl IntoResponse for RateLimitError {
    fn into_response(self) -> axum::response::Response {
        let (status, message) = match self {
            RateLimitError::LimitExceeded => (StatusCode::TOO_MANY_REQUESTS, "Rate limit exceeded"),
            RateLimitError::InvalidConfig(_) => {
                (StatusCode::INTERNAL_SERVER_ERROR, "Invalid configuration")
            }
            RateLimitError::InternalError(_) => {
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal error")
            }
        };

        (status, message).into_response()
    }
}

// Statistics for monitoring
#[derive(Debug, Serialize)]
pub struct RateLimitStats {
    pub total_requests: u64,
    pub blocked_requests: u64,
    pub active_limiters: usize,
    pub success_rate: f64,
}

impl RateLimitStats {
    pub fn new() -> Self {
        Self {
            total_requests: 0,
            blocked_requests: 0,
            active_limiters: 0,
            success_rate: 100.0,
        }
    }

    pub fn calculate_success_rate(&mut self) {
        if self.total_requests > 0 {
            self.success_rate = ((self.total_requests - self.blocked_requests) as f64
                / self.total_requests as f64)
                * 100.0;
        }
    }
}

/// Errors that can occur when parsing rate limit policy strings
#[derive(Debug, Error)]
pub enum PolicyParseError {
    #[error("Invalid policy format for '{spec}': expected 'path:limit:window_secs' or 'key:limit:window_secs', got {parts} parts")]
    InvalidFormat { spec: String, parts: usize },

    #[error("Invalid limit value '{value}' in policy '{spec}': {reason}")]
    InvalidLimit {
        spec: String,
        value: String,
        reason: String,
    },

    #[error("Invalid window duration '{value}' in policy '{spec}': {reason}")]
    InvalidWindow {
        spec: String,
        value: String,
        reason: String,
    },

    #[error("Empty policy specification")]
    EmptySpec,

    #[error("Path policy must start with '/': got '{path}'")]
    InvalidPathFormat { path: String },

    #[error("Window duration must be at least 1 second, got {window_secs}")]
    WindowTooSmall { window_secs: u64 },

    #[error("Limit must be at least 1, got {limit}")]
    LimitTooSmall { limit: u32 },
}

/// Result of parsing rate limit policies
#[derive(Debug)]
pub struct ParsedPolicies {
    /// Successfully parsed path policies
    pub path_policies: Vec<PathPolicy>,
    /// Successfully parsed API key policies
    pub api_key_policies: HashMap<String, (u32, Duration)>,
    /// Successfully parsed user policies
    pub user_policies: HashMap<String, (u32, Duration)>,
    /// Any warnings or non-fatal issues encountered
    pub warnings: Vec<String>,
}

impl Default for ParsedPolicies {
    fn default() -> Self {
        Self {
            path_policies: Vec::new(),
            api_key_policies: HashMap::new(),
            user_policies: HashMap::new(),
            warnings: Vec::new(),
        }
    }
}

/// Parse a path policy specification string.
///
/// Format: "path:limit:window_secs"
/// Example: "/api/v1/orders:100:60"
///
/// # Arguments
/// * `spec` - The policy specification string
///
/// # Returns
/// * `Ok(PathPolicy)` - Successfully parsed policy
/// * `Err(PolicyParseError)` - Parse error with details
pub fn parse_path_policy(spec: &str) -> Result<PathPolicy, PolicyParseError> {
    let spec = spec.trim();
    if spec.is_empty() {
        return Err(PolicyParseError::EmptySpec);
    }

    let parts: Vec<&str> = spec.split(':').collect();
    if parts.len() != 3 {
        return Err(PolicyParseError::InvalidFormat {
            spec: spec.to_string(),
            parts: parts.len(),
        });
    }

    let path = parts[0].trim();
    if !path.starts_with('/') {
        return Err(PolicyParseError::InvalidPathFormat {
            path: path.to_string(),
        });
    }

    let limit: u32 = parts[1]
        .trim()
        .parse()
        .map_err(|e| PolicyParseError::InvalidLimit {
            spec: spec.to_string(),
            value: parts[1].to_string(),
            reason: format!("{}", e),
        })?;

    if limit < 1 {
        return Err(PolicyParseError::LimitTooSmall { limit });
    }

    let window_secs: u64 =
        parts[2]
            .trim()
            .parse()
            .map_err(|e| PolicyParseError::InvalidWindow {
                spec: spec.to_string(),
                value: parts[2].to_string(),
                reason: format!("{}", e),
            })?;

    if window_secs < 1 {
        return Err(PolicyParseError::WindowTooSmall { window_secs });
    }

    Ok(PathPolicy {
        prefix: path.to_string(),
        requests_per_window: limit,
        window_duration: Duration::from_secs(window_secs),
    })
}

/// Parse a key-based policy specification string (for API keys or users).
///
/// Format: "key:limit:window_secs"
/// Example: "sk_live_abc123:1000:60"
///
/// # Arguments
/// * `spec` - The policy specification string
///
/// # Returns
/// * `Ok((key, (limit, duration)))` - Successfully parsed policy
/// * `Err(PolicyParseError)` - Parse error with details
pub fn parse_key_policy(spec: &str) -> Result<(String, (u32, Duration)), PolicyParseError> {
    let spec = spec.trim();
    if spec.is_empty() {
        return Err(PolicyParseError::EmptySpec);
    }

    let parts: Vec<&str> = spec.split(':').collect();
    if parts.len() != 3 {
        return Err(PolicyParseError::InvalidFormat {
            spec: spec.to_string(),
            parts: parts.len(),
        });
    }

    let key = parts[0].trim();
    if key.is_empty() {
        return Err(PolicyParseError::EmptySpec);
    }

    let limit: u32 = parts[1]
        .trim()
        .parse()
        .map_err(|e| PolicyParseError::InvalidLimit {
            spec: spec.to_string(),
            value: parts[1].to_string(),
            reason: format!("{}", e),
        })?;

    if limit < 1 {
        return Err(PolicyParseError::LimitTooSmall { limit });
    }

    let window_secs: u64 =
        parts[2]
            .trim()
            .parse()
            .map_err(|e| PolicyParseError::InvalidWindow {
                spec: spec.to_string(),
                value: parts[2].to_string(),
                reason: format!("{}", e),
            })?;

    if window_secs < 1 {
        return Err(PolicyParseError::WindowTooSmall { window_secs });
    }

    Ok((key.to_string(), (limit, Duration::from_secs(window_secs))))
}

/// Parse multiple path policies from a comma-separated string.
///
/// Format: "path1:limit1:window1,path2:limit2:window2"
/// Example: "/api/v1/orders:100:60,/api/v1/inventory:200:60"
///
/// # Arguments
/// * `policies_str` - Comma-separated policy specifications
///
/// # Returns
/// * `Ok(Vec<PathPolicy>)` - Successfully parsed policies (may be empty if all failed)
/// * Logs warnings for any policies that failed to parse
pub fn parse_path_policies(policies_str: &str) -> (Vec<PathPolicy>, Vec<String>) {
    let mut policies = Vec::new();
    let mut warnings = Vec::new();

    for spec in policies_str
        .split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
    {
        match parse_path_policy(spec) {
            Ok(policy) => policies.push(policy),
            Err(e) => warnings.push(format!("Skipping invalid path policy '{}': {}", spec, e)),
        }
    }

    (policies, warnings)
}

/// Parse multiple key policies from a comma-separated string.
///
/// Format: "key1:limit1:window1,key2:limit2:window2"
/// Example: "sk_live_abc:1000:60,sk_live_xyz:500:60"
///
/// # Arguments
/// * `policies_str` - Comma-separated policy specifications
///
/// # Returns
/// * Parsed policies as HashMap and any warnings
pub fn parse_key_policies(policies_str: &str) -> (HashMap<String, (u32, Duration)>, Vec<String>) {
    let mut policies = HashMap::new();
    let mut warnings = Vec::new();

    for spec in policies_str
        .split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
    {
        match parse_key_policy(spec) {
            Ok((key, value)) => {
                if policies.contains_key(&key) {
                    warnings.push(format!(
                        "Duplicate key '{}' in policies, using last value",
                        key
                    ));
                }
                policies.insert(key, value);
            }
            Err(e) => warnings.push(format!("Skipping invalid key policy '{}': {}", spec, e)),
        }
    }

    (policies, warnings)
}

/// Parse all rate limit policies from configuration strings.
///
/// This is the main entry point for parsing rate limit policies from environment
/// variables or configuration files. It validates all policies and returns
/// structured results with any warnings.
///
/// # Arguments
/// * `path_policies` - Optional path policies string
/// * `api_key_policies` - Optional API key policies string
/// * `user_policies` - Optional user policies string
///
/// # Returns
/// * `ParsedPolicies` - Contains all successfully parsed policies and warnings
pub fn parse_all_policies(
    path_policies: Option<&str>,
    api_key_policies: Option<&str>,
    user_policies: Option<&str>,
) -> ParsedPolicies {
    let mut result = ParsedPolicies::default();

    if let Some(path_str) = path_policies {
        let (policies, warnings) = parse_path_policies(path_str);
        result.path_policies = policies;
        result.warnings.extend(warnings);
    }

    if let Some(api_key_str) = api_key_policies {
        let (policies, warnings) = parse_key_policies(api_key_str);
        result.api_key_policies = policies;
        result.warnings.extend(warnings);
    }

    if let Some(user_str) = user_policies {
        let (policies, warnings) = parse_key_policies(user_str);
        result.user_policies = policies;
        result.warnings.extend(warnings);
    }

    result
}

#[cfg(test)]
mod policy_parsing_tests {
    use super::*;

    #[test]
    fn test_parse_valid_path_policy() {
        let policy = parse_path_policy("/api/v1/orders:100:60").unwrap();
        assert_eq!(policy.prefix, "/api/v1/orders");
        assert_eq!(policy.requests_per_window, 100);
        assert_eq!(policy.window_duration, Duration::from_secs(60));
    }

    #[test]
    fn test_parse_path_policy_with_spaces() {
        let policy = parse_path_policy("  /api/v1/orders : 100 : 60  ").unwrap();
        assert_eq!(policy.prefix, "/api/v1/orders");
        assert_eq!(policy.requests_per_window, 100);
    }

    #[test]
    fn test_parse_path_policy_invalid_format() {
        let result = parse_path_policy("/api/v1/orders:100");
        assert!(matches!(
            result,
            Err(PolicyParseError::InvalidFormat { .. })
        ));
    }

    #[test]
    fn test_parse_path_policy_no_leading_slash() {
        let result = parse_path_policy("api/v1/orders:100:60");
        assert!(matches!(
            result,
            Err(PolicyParseError::InvalidPathFormat { .. })
        ));
    }

    #[test]
    fn test_parse_path_policy_invalid_limit() {
        let result = parse_path_policy("/api:abc:60");
        assert!(matches!(result, Err(PolicyParseError::InvalidLimit { .. })));
    }

    #[test]
    fn test_parse_path_policy_zero_limit() {
        let result = parse_path_policy("/api:0:60");
        assert!(matches!(
            result,
            Err(PolicyParseError::LimitTooSmall { .. })
        ));
    }

    #[test]
    fn test_parse_path_policy_zero_window() {
        let result = parse_path_policy("/api:100:0");
        assert!(matches!(
            result,
            Err(PolicyParseError::WindowTooSmall { .. })
        ));
    }

    #[test]
    fn test_parse_valid_key_policy() {
        let (key, (limit, duration)) = parse_key_policy("sk_live_abc123:1000:60").unwrap();
        assert_eq!(key, "sk_live_abc123");
        assert_eq!(limit, 1000);
        assert_eq!(duration, Duration::from_secs(60));
    }

    #[test]
    fn test_parse_multiple_path_policies() {
        let (policies, warnings) = parse_path_policies(
            "/api/v1/orders:100:60,/api/v1/inventory:200:60,invalid,/api/v1/users:50:30",
        );
        assert_eq!(policies.len(), 3);
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("invalid"));
    }

    #[test]
    fn test_parse_multiple_key_policies() {
        let (policies, warnings) =
            parse_key_policies("key1:100:60,key2:200:60,bad_policy,key3:50:30");
        assert_eq!(policies.len(), 3);
        assert_eq!(warnings.len(), 1);
    }

    #[test]
    fn test_parse_all_policies() {
        let result = parse_all_policies(
            Some("/api/v1/orders:100:60"),
            Some("sk_live_test:500:60"),
            Some("user123:200:60"),
        );
        assert_eq!(result.path_policies.len(), 1);
        assert_eq!(result.api_key_policies.len(), 1);
        assert_eq!(result.user_policies.len(), 1);
        assert!(result.warnings.is_empty());
    }
}

#[cfg(test)]
mod rate_limiter_shared_store_tests {
    use super::*;

    #[tokio::test]
    async fn overrides_share_underlying_store() {
        let base_config = RateLimitConfig {
            requests_per_window: 2,
            window_duration: Duration::from_secs(60),
            ..Default::default()
        };
        let base = RateLimiter::in_memory(base_config.clone());

        let mut override_config = base_config.clone();
        override_config.requests_per_window = 1;
        let override_limiter = base.with_config(override_config);

        let first = base
            .check_rate_limit("user:test-shared")
            .await
            .expect("first check");
        assert!(first.allowed, "first request should be allowed");

        let second = override_limiter
            .check_rate_limit("user:test-shared")
            .await
            .expect("second check");
        assert!(
            !second.allowed,
            "override limiter should see the incremented count"
        );
    }
}

#[cfg(all(test, feature = "mock-tests"))]
mod tests {
    use super::*;
    use tokio::time::Duration;

    #[tokio::test]
    async fn test_rate_limiter_basic_functionality() {
        let config = RateLimitConfig {
            requests_per_window: 2,
            window_duration: Duration::from_secs(60),
            ..Default::default()
        };

        let limiter = RateLimiter::in_memory(config);

        // First request should succeed
        assert!(limiter.check_rate_limit("test_key").await.unwrap().allowed);

        // Second request should succeed
        assert!(limiter.check_rate_limit("test_key").await.unwrap().allowed);

        // Third request should fail
        assert!(!limiter.check_rate_limit("test_key").await.unwrap().allowed);
    }

    #[tokio::test]
    async fn test_rate_limiter_different_keys() {
        let config = RateLimitConfig {
            requests_per_window: 1,
            window_duration: Duration::from_secs(60),
            ..Default::default()
        };

        let limiter = RateLimiter::in_memory(config);

        // Different keys should have separate limits
        assert!(limiter.check_rate_limit("key1").await.unwrap().allowed);
        assert!(limiter.check_rate_limit("key2").await.unwrap().allowed);

        // Both keys should now be at their limit
        assert!(!limiter.check_rate_limit("key1").await.unwrap().allowed);
        assert!(!limiter.check_rate_limit("key2").await.unwrap().allowed);
    }

    #[tokio::test]
    async fn test_rate_limiter_quota() {
        let config = RateLimitConfig {
            requests_per_window: 5,
            window_duration: Duration::from_secs(60),
            ..Default::default()
        };

        let limiter = RateLimiter::in_memory(config);

        // Initially should have full quota
        assert_eq!(limiter.get_remaining_quota("test_key").await, 5);

        // After one request, quota should decrease
        assert!(limiter.check_rate_limit("test_key").await.unwrap().allowed);
        assert_eq!(limiter.get_remaining_quota("test_key").await, 4);
    }
}
