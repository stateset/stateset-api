use axum::{
    body::Body,
    http::{Request, StatusCode, Response},
    response::IntoResponse,
};
use redis::{AsyncCommands, Script};
use slog::{info, warn, Logger};
use std::{
    sync::Arc,
    time::Duration,
};
use thiserror::Error;
use tower::{Layer, Service};
use tokio::sync::Semaphore;
use futures::future::BoxFuture;
use std::task::{Context as TaskContext, Poll};

/// Rate limiting errors
#[derive(Error, Debug)]
pub enum RateLimitError {
    #[error("Redis connection error: {0}")]
    RedisConnection(#[from] redis::RedisError),
    #[error("Rate limit exceeded")]
    Exceeded,
    #[error("Internal rate limiter error: {0}")]
    Internal(String),
}

impl IntoResponse for RateLimitError {
    fn into_response(self) -> Response<Body> {
        match self {
            Self::Exceeded => (
                StatusCode::TOO_MANY_REQUESTS,
                "Rate limit exceeded".to_string(),
            ).into_response(),
            _ => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal server error".to_string(),
            ).into_response(),
        }
    }
}

/// Configuration for rate limiter
#[derive(Clone)]
pub struct RateLimitConfig {
    max_requests: usize,
    window: Duration,
    key_prefix: String,
}

/// Rate limiter implementation
#[derive(Clone)]
pub struct RateLimiter {
    redis: Arc<redis::aio::ConnectionManager>,
    config: RateLimitConfig,
    logger: Logger,
    semaphore: Arc<Semaphore>,
    script: Arc<Script>,
}

impl RateLimiter {
    /// Creates a new RateLimiter instance
    pub async fn new(
        redis_url: &str,
        key_prefix: &str,
        max_requests: usize,
        window: Duration,
        logger: Logger,
    ) -> Result<Self, RateLimitError> {
        let client = redis::Client::open(redis_url)
            .map_err(RateLimitError::RedisConnection)?;
        let redis = redis::aio::ConnectionManager::new(client)
            .await
            .map_err(RateLimitError::RedisConnection)?;

        let script = Script::new(r#"
            local current = redis.call("INCR", KEYS[1])
            if tonumber(current) == 1 then
                redis.call("EXPIRE", KEYS[1], ARGV[1])
            end
            return current
        "#);

        Ok(Self {
            redis: Arc::new(redis),
            config: RateLimitConfig {
                max_requests,
                window,
                key_prefix: key_prefix.to_string(),
            },
            logger,
            semaphore: Arc::new(Semaphore::new(100)), // Limit concurrent evaluations
            script: Arc::new(script),
        })
    }

    /// Checks if a key exceeds the rate limit
    pub async fn check(&self, key: &str) -> Result<bool, RateLimitError> {
        let full_key = format!("{}:{}", self.config.key_prefix, key);
        let _permit = self.semaphore.acquire().await
            .map_err(|e| RateLimitError::Internal(e.to_string()))?;

        let current: usize = self.script
            .key(&full_key)
            .arg(self.config.window.as_secs().to_string())
            .invoke_async(&mut self.redis.clone())
            .await
            .map_err(RateLimitError::RedisConnection)?;

        let exceeded = current > self.config.max_requests;
        if exceeded {
            warn!(self.logger, "Rate limit exceeded";
                "key" => key,
                "current" => current,
                "max" => self.config.max_requests
            );
        } else {
            info!(self.logger, "Rate limit check";
                "key" => key,
                "current" => current,
                "max" => self.config.max_requests
            );
        }

        Ok(exceeded)
    }
}

/// Rate limit middleware layer
#[derive(Clone)]
pub struct RateLimitLayer<F> {
    limiter: Arc<RateLimiter>,
    key_extractor: F,
}

impl<F> RateLimitLayer<F>
where
    F: Fn(&Request<Body>) -> String + Send + Sync + 'static,
{
    pub fn new(limiter: Arc<RateLimiter>, key_extractor: F) -> Self {
        Self { limiter, key_extractor }
    }
}

impl<S, F> Layer<S> for RateLimitLayer<F>
where
    F: Fn(&Request<Body>) -> String + Send + Sync + 'static,
{
    type Service = RateLimitService<S, F>;

    fn layer(&self, inner: S) -> Self::Service {
        RateLimitService {
            inner,
            limiter: self.limiter.clone(),
            key_extractor: self.key_extractor.clone(),
        }
    }
}

/// Rate limit service implementation
#[derive(Clone)]
pub struct RateLimitService<S, F> {
    inner: S,
    limiter: Arc<RateLimiter>,
    key_extractor: F,
}

impl<S, F> Service<Request<Body>> for RateLimitService<S, F>
where
    S: Service<Request<Body>, Response = Response<Body>, Error = axum::Error> + Send + Clone + 'static,
    S::Future: Send + 'static,
    F: Fn(&Request<Body>) -> String + Send + Sync + 'static,
{
    type Response = Response<Body>;
    type Error = axum::Error;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut TaskContext<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        let limiter = self.limiter.clone();
        let key = (self.key_extractor)(&req);
        let inner = self.inner.clone();

        Box::pin(async move {
            match limiter.check(&key).await {
                Ok(true) => Ok(RateLimitError::Exceeded.into_response()),
                Ok(false) => inner.call(req).await,
                Err(e) => {
                    warn!(limiter.logger, "Rate limiter error"; "error" => %e);
                    Ok(e.into_response())
                }
            }
        })
    }
}

/// Default key extractor using client IP
pub fn ip_key_extractor(req: &Request<Body>) -> String {
    req.headers()
        .get("X-Forwarded-For")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("unknown")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{routing::get, Router};
    use slog::{o, Drain, Logger};
    use slog_term::TermDecorator;
    use slog_async::Async;
    use tower::ServiceExt;
    use tokio::time::{sleep, Duration};

    fn setup_logger() -> Logger {
        let decorator = TermDecorator::new().build();
        let drain = slog_term::CompactFormat::new(decorator).build().fuse();
        let drain = Async::new(drain).build().fuse();
        Logger::root(drain, o!())
    }

    async fn dummy_handler() -> impl IntoResponse {
        "OK"
    }

    #[tokio::test]
    async fn test_rate_limiter() {
        let logger = setup_logger();
        let limiter = RateLimiter::new(
            "redis://localhost:6379",
            "test",
            2, // 2 requests
            Duration::from_secs(1),
            logger,
        ).await.unwrap();

        let layer = RateLimitLayer::new(
            Arc::new(limiter),
            |_| "test_key".to_string(),
        );

        let app = Router::new()
            .route("/", get(dummy_handler))
            .layer(layer);

        let req = Request::builder()
            .uri("/")
            .body(Body::empty())
            .unwrap();

        // First request - should pass
        let resp = app.clone().oneshot(req.clone()).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        // Second request - should pass
        let resp = app.clone().oneshot(req.clone()).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        // Third request - should be rate limited
        let resp = app.clone().oneshot(req.clone()).await.unwrap();
        assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);

        // Wait for window to expire
        sleep(Duration::from_secs(2)).await;

        // After window - should pass again
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }
}