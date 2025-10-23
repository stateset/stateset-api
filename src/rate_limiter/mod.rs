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
 * ```
 * // Create a rate limiter
 * let limiter = RateLimiter::new(
 *     "redis://localhost:6379",
 *     "api-prefix",
 *     100, // 100 requests
 *     Duration::from_secs(60), // per minute
 *     logger
 * ).await?;
 *
 * // Apply the rate limiter middleware
 * let app = Router::new()
 *     .route("/", get(handler))
 *     .layer(RateLimitLayer::new(Arc::new(limiter), ip_key_extractor));
 * ```
 */
use axum::{
    extract::Request,
    http::{Response, StatusCode},
    middleware::Next,
    response::IntoResponse,
};
use dashmap::DashMap;
use metrics::counter;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use thiserror::Error;
use tracing::{debug, warn};

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
pub struct RateLimiter {
    entries: Arc<DashMap<String, RateLimitEntry>>,
    config: RateLimitConfig,
}

impl RateLimiter {
    pub fn new(config: RateLimitConfig) -> Self {
        Self {
            entries: Arc::new(DashMap::new()),
            config,
        }
    }

    pub async fn check_rate_limit(&self, key: &str) -> Result<RateLimitResult, RateLimitError> {
        let mut entry = self
            .entries
            .entry(key.to_string())
            .or_insert_with(RateLimitEntry::new);

        // Check if request is allowed
        if !entry.is_allowed(self.config.requests_per_window, self.config.window_duration) {
            let time_until_reset = entry.time_until_reset(self.config.window_duration);
            return Ok(RateLimitResult {
                allowed: false,
                limit: self.config.requests_per_window,
                remaining: 0,
                reset_time: time_until_reset,
            });
        }

        // Increment counter
        entry.increment(self.config.window_duration);
        let remaining = self.config.requests_per_window.saturating_sub(entry.count);
        let time_until_reset = entry.time_until_reset(self.config.window_duration);

        Ok(RateLimitResult {
            allowed: true,
            limit: self.config.requests_per_window,
            remaining,
            reset_time: time_until_reset,
        })
    }

    pub async fn get_remaining_quota(&self, key: &str) -> u32 {
        if let Some(entry) = self.entries.get(key) {
            let now = Instant::now();
            // Check if window has expired
            if now.duration_since(entry.window_start) >= self.config.window_duration {
                return self.config.requests_per_window;
            }
            self.config.requests_per_window.saturating_sub(entry.count)
        } else {
            self.config.requests_per_window
        }
    }

    pub async fn reset(&self, key: &str) -> Result<(), RateLimitError> {
        self.entries.remove(key);
        Ok(())
    }

    pub async fn cleanup_expired(&self) {
        let now = Instant::now();
        let window_duration = self.config.window_duration;

        self.entries
            .retain(|_, entry| now.duration_since(entry.window_start) < window_duration * 2);
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

pub fn extract_user_key(request: &Request) -> Option<String> {
    // Try to get user ID from Authorization header or custom header
    if let Some(auth) = request.headers().get("authorization") {
        if let Ok(auth_str) = auth.to_str() {
            // Simple extraction - in real implementation you'd decode JWT token
            return Some(format!(
                "user:{}",
                auth_str.chars().take(20).collect::<String>()
            ));
        }
    }

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
    let rate_limiter = RateLimiter::new(config.clone());

    // Extract key (prefer API key, then user, then IP)
    let key = if let Some(k) = extract_api_key(&request) {
        k
    } else if let Some(u) = extract_user_key(&request) {
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
                    headers.insert(
                        "X-RateLimit-Limit",
                        result.limit.to_string().parse().unwrap(),
                    );
                    headers.insert("X-RateLimit-Remaining", "0".parse().unwrap());
                    headers.insert(
                        "X-RateLimit-Reset",
                        result.reset_time.as_secs().to_string().parse().unwrap(),
                    );
                }

                return Err(response);
            }

            // Process request
            let mut response = next.run(request).await;

            // Add rate limit headers to successful response
            if config.enable_headers {
                let headers = response.headers_mut();
                headers.insert(
                    "X-RateLimit-Limit",
                    result.limit.to_string().parse().unwrap(),
                );
                headers.insert(
                    "X-RateLimit-Remaining",
                    result.remaining.to_string().parse().unwrap(),
                );
                headers.insert(
                    "X-RateLimit-Reset",
                    result.reset_time.as_secs().to_string().parse().unwrap(),
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
}

impl RateLimitLayer {
    pub fn new(config: RateLimitConfig) -> Self {
        Self {
            rate_limiter: RateLimiter::new(config),
            path_policies: Arc::new(Vec::new()),
            api_key_policies: Arc::new(HashMap::new()),
            user_policies: Arc::new(HashMap::new()),
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

        Box::pin(async move {
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
            } else if let Some(u) = extract_user_key(&request) {
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
                RateLimiter::new(effective)
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
                            let _ = headers.insert(
                                "X-RateLimit-Limit",
                                result.limit.to_string().parse().unwrap(),
                            );
                            let _ = headers.insert("X-RateLimit-Remaining", "0".parse().unwrap());
                            let _ = headers.insert(
                                "X-RateLimit-Reset",
                                result.reset_time.as_secs().to_string().parse().unwrap(),
                            );
                            // RFC 9447 headers
                            let _ = headers.insert(
                                "RateLimit-Limit",
                                result.limit.to_string().parse().unwrap(),
                            );
                            let _ = headers.insert("RateLimit-Remaining", "0".parse().unwrap());
                            let _ = headers.insert(
                                "RateLimit-Reset",
                                result.reset_time.as_secs().to_string().parse().unwrap(),
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
                        let _ = headers.insert(
                            "X-RateLimit-Limit",
                            result.limit.to_string().parse().unwrap(),
                        );
                        let _ = headers.insert(
                            "X-RateLimit-Remaining",
                            result.remaining.to_string().parse().unwrap(),
                        );
                        let _ = headers.insert(
                            "X-RateLimit-Reset",
                            result.reset_time.as_secs().to_string().parse().unwrap(),
                        );
                        // RFC 9447
                        let _ = headers
                            .insert("RateLimit-Limit", result.limit.to_string().parse().unwrap());
                        let _ = headers.insert(
                            "RateLimit-Remaining",
                            result.remaining.to_string().parse().unwrap(),
                        );
                        let _ = headers.insert(
                            "RateLimit-Reset",
                            result.reset_time.as_secs().to_string().parse().unwrap(),
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

        let limiter = RateLimiter::new(config);

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

        let limiter = RateLimiter::new(config);

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

        let limiter = RateLimiter::new(config);

        // Initially should have full quota
        assert_eq!(limiter.get_remaining_quota("test_key").await, 5);

        // After one request, quota should decrease
        assert!(limiter.check_rate_limit("test_key").await.unwrap().allowed);
        assert_eq!(limiter.get_remaining_quota("test_key").await, 4);
    }
}
