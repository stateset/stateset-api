use crate::auth::ApiKeyInfo;
use axum::{
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};
use governor::{
    clock::DefaultClock, state::keyed::DashMapStateStore, Quota, RateLimiter as GovernorRateLimiter,
};
use std::num::NonZeroU32;
use std::sync::Arc;

/// Rate limiter using governor
pub struct RateLimiter {
    limiter: Arc<GovernorRateLimiter<String, DashMapStateStore<String>, DefaultClock>>,
}

impl RateLimiter {
    pub fn new(requests_per_minute: u32) -> Self {
        let quota = Quota::per_minute(NonZeroU32::new(requests_per_minute).unwrap());
        let limiter = Arc::new(GovernorRateLimiter::keyed(quota));

        Self { limiter }
    }

    pub fn check(&self, key: &str) -> bool {
        let key_owned = key.to_owned();
        self.limiter.check_key(&key_owned).is_ok()
    }
}

impl Clone for RateLimiter {
    fn clone(&self) -> Self {
        Self {
            limiter: Arc::clone(&self.limiter),
        }
    }
}

/// Rate limiting middleware
pub async fn rate_limit_middleware(
    limiter: axum::extract::State<RateLimiter>,
    request: Request,
    next: Next,
) -> Response {
    let key = request
        .extensions()
        .get::<ApiKeyInfo>()
        .map(|info| format!("api_key:{}", info.key))
        .or_else(|| {
            request
                .headers()
                .get("x-forwarded-for")
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.split(',').next())
                .map(|ip| format!("ip:{}", ip.trim()))
        })
        .unwrap_or_else(|| "global".to_string());

    if !limiter.check(&key) {
        return (
            StatusCode::TOO_MANY_REQUESTS,
            axum::Json(serde_json::json!({
                "type": "rate_limit_exceeded",
                "code": "rate_limit_exceeded",
                "message": "Too many requests. Please try again later."
            })),
        )
            .into_response();
    }

    next.run(request).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_rate_limiter() {
        let limiter = RateLimiter::new(60); // 60 requests per minute

        // First request should succeed
        assert!(limiter.check("api_key:test"));

        // Exceed the quota quickly by looping
        for _ in 0..60 {
            let _ = limiter.check("api_key:test");
        }

        assert!(!limiter.check("api_key:test"));

        // Different key should have independent quota
        assert!(limiter.check("api_key:other"));
    }
}
