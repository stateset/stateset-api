use axum::{
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};
use governor::{
    clock::DefaultClock,
    state::{InMemoryState, NotKeyed},
    Quota, RateLimiter as GovernorRateLimiter,
};
use std::num::NonZeroU32;
use std::sync::Arc;

/// Rate limiter using governor
pub struct RateLimiter {
    limiter: Arc<GovernorRateLimiter<NotKeyed, InMemoryState, DefaultClock>>,
}

impl RateLimiter {
    pub fn new(requests_per_minute: u32) -> Self {
        let quota = Quota::per_minute(NonZeroU32::new(requests_per_minute).unwrap());
        let limiter = Arc::new(GovernorRateLimiter::direct(quota));

        Self { limiter }
    }

    pub fn check(&self) -> bool {
        self.limiter.check().is_ok()
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
    if !limiter.check() {
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

    #[test]
    fn test_rate_limiter() {
        let limiter = RateLimiter::new(60); // 60 requests per minute

        // First request should succeed
        assert!(limiter.check());
    }
}
