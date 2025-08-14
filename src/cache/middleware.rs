use axum::{
    body::Body,
    http::{header, HeaderValue, Method, StatusCode, Request},
    middleware::Next,
    response::{IntoResponse, Response},
};
use bytes::Bytes;
use dashmap::DashMap;
use http_body_util::BodyExt;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tower::{Layer, Service};
use tracing::{debug, info, warn};

use super::{Cache, CacheError};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
    pub timestamp: u64,
}

impl CachedResponse {
    pub fn new(status: StatusCode, headers: &header::HeaderMap, body: Vec<u8>) -> Self {
        let headers = headers
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
            .collect();
        
        Self {
            status: status.as_u16(),
            headers,
            body,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        }
    }
}

impl IntoResponse for CachedResponse {
    fn into_response(self) -> Response {
        let mut response = Response::builder().status(self.status);
        
        for (key, value) in &self.headers {
            if let Ok(header_value) = HeaderValue::from_str(value) {
                response = response.header(key, header_value);
            }
        }
        
        response.body(Body::from(self.body)).unwrap()
    }
}

#[derive(Debug, Clone)]
pub struct CacheOptions {
    pub default_ttl: Duration,
    pub max_body_size: usize,
    pub vary_headers: Vec<String>,
    pub cache_control_respect: bool,
    pub stale_while_revalidate: Option<Duration>,
    pub exclude_patterns: Vec<String>,
}

impl Default for CacheOptions {
    fn default() -> Self {
        Self {
            default_ttl: Duration::from_secs(300), // 5 minutes
            max_body_size: 1024 * 1024, // 1MB
            vary_headers: vec!["Accept".to_string(), "Accept-Encoding".to_string()],
            cache_control_respect: true,
            stale_while_revalidate: Some(Duration::from_secs(86400)), // 24 hours
            exclude_patterns: vec![],
        }
    }
}

#[derive(Clone)]
pub struct HttpCache {
    pub cache: Arc<Cache>,
    pub options: CacheOptions,
    invalidation_map: Arc<DashMap<String, Vec<String>>>,
}

impl HttpCache {
    pub fn new(cache: Arc<Cache>, options: CacheOptions) -> Self {
        Self {
            cache,
            options,
            invalidation_map: Arc::new(DashMap::new()),
        }
    }

    fn generate_cache_key(&self, request: &Request<Body>) -> String {
        let mut key_parts = vec![
            request.method().to_string(),
            request.uri().path().to_string(),
        ];

        if let Some(query) = request.uri().query() {
            key_parts.push(query.to_string());
        }

        // Add vary headers to cache key
        for header_name in &self.options.vary_headers {
            if let Some(header_value) = request.headers().get(header_name) {
                if let Ok(value_str) = header_value.to_str() {
                    key_parts.push(format!("{}:{}", header_name, value_str));
                }
            }
        }

        format!("http_cache:{}", key_parts.join(":"))
    }

    fn should_cache_request(&self, request: &Request<Body>) -> bool {
        // Only cache GET requests
        if request.method() != Method::GET {
            return false;
        }

        // Check exclude patterns
        let path = request.uri().path();
        for pattern in &self.options.exclude_patterns {
            if path.contains(pattern) {
                return false;
            }
        }

        true
    }

    fn should_cache_response(&self, response: &Response) -> bool {
        let status = response.status();
        
        // Only cache successful responses
        if !status.is_success() {
            return false;
        }

        // Check cache control headers if enabled
        if self.options.cache_control_respect {
            if let Some(cache_control) = response.headers().get(header::CACHE_CONTROL) {
                if let Ok(cache_control_str) = cache_control.to_str() {
                    if cache_control_str.contains("no-cache") || cache_control_str.contains("no-store") {
                        return false;
                    }
                }
            }
        }

        true
    }

    fn extract_ttl_from_response(&self, response: &Response) -> Option<Duration> {
        if !self.options.cache_control_respect {
            return Some(self.options.default_ttl);
        }

        if let Some(cache_control) = response.headers().get(header::CACHE_CONTROL) {
            if let Ok(cache_control_str) = cache_control.to_str() {
                for directive in cache_control_str.split(',') {
                    let directive = directive.trim();
                    if let Some(max_age) = directive.strip_prefix("max-age=") {
                        if let Ok(seconds) = max_age.parse::<u64>() {
                            return Some(Duration::from_secs(seconds));
                        }
                    }
                }
            }
        }

        Some(self.options.default_ttl)
    }

    pub async fn invalidate_pattern(&self, pattern: &str) -> Result<(), CacheError> {
        // This is a simple implementation - in a real system you'd want more sophisticated pattern matching
        warn!("Pattern-based cache invalidation not fully implemented: {}", pattern);
        Ok(())
    }

    pub async fn get_cached_response(&self, cache_key: &str) -> Option<CachedResponse> {
        match self.cache.get(cache_key).await {
            Ok(Some(cached_data)) => {
                match serde_json::from_str::<CachedResponse>(&cached_data) {
                    Ok(cached_response) => {
                        debug!("Cache hit for key: {}", cache_key);
                        Some(cached_response)
                    }
                    Err(e) => {
                        warn!("Failed to deserialize cached response: {}", e);
                        None
                    }
                }
            }
            Ok(None) => {
                debug!("Cache miss for key: {}", cache_key);
                None
            }
            Err(e) => {
                warn!("Cache error: {}", e);
                None
            }
        }
    }

    pub async fn store_response(
        &self,
        cache_key: &str,
        response: &Response,
        body: &[u8],
        ttl: Option<Duration>,
    ) -> Result<(), CacheError> {
        if body.len() > self.options.max_body_size {
            debug!("Response body too large to cache: {} bytes", body.len());
            return Ok(());
        }

        let cached_response = CachedResponse::new(
            response.status(),
            response.headers(),
            body.to_vec(),
        );

        let serialized = serde_json::to_string(&cached_response)
            .map_err(CacheError::Serialization)?;

        self.cache.set(cache_key, &serialized, ttl).await?;
        debug!("Stored response in cache with key: {}", cache_key);
        
        Ok(())
    }
}

// Middleware function
pub async fn cache_middleware(
    request: Request<Body>,
    next: Next,
) -> Result<Response, Response> {
    // For now, just pass through without caching since we need the cache instance
    // In a real implementation, this would be configured with the cache instance
    let response = next.run(request).await;
    Ok(response)
}

// Cache layer for axum
#[derive(Clone)]
pub struct CacheLayer {
    http_cache: HttpCache,
}

impl CacheLayer {
    pub fn new(cache: Arc<Cache>, options: CacheOptions) -> Self {
        Self {
            http_cache: HttpCache::new(cache, options),
        }
    }
}

impl<S> Layer<S> for CacheLayer {
    type Service = CacheService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        CacheService {
            inner,
            http_cache: self.http_cache.clone(),
        }
    }
}

#[derive(Clone)]
pub struct CacheService<S> {
    inner: S,
    http_cache: HttpCache,
}

impl<S> Service<Request<Body>> for CacheService<S>
where
    S: Service<Request<Body>, Response = Response> + Clone + Send + 'static,
    S::Future: Send + 'static,
{
    type Response = Response;
    type Error = S::Error;
    type Future = std::pin::Pin<Box<dyn std::future::Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut std::task::Context<'_>) -> std::task::Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, request: Request<Body>) -> Self::Future {
        let cache = self.http_cache.clone();
        let mut inner = self.inner.clone();

        Box::pin(async move {
            // Temporarily bypass caching to avoid Send bounds; just pass through
            inner.call(request).await
        })
    }
}
