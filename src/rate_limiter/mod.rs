use redis::{Client as RedisClient, AsyncCommands};
use std::time::Duration;
use axum::{
    http::{Request, StatusCode},
    response::{IntoResponse, Response},
    body::Body,
};
use std::sync::Arc;
use slog::{Logger, info, error};
use thiserror::Error;
use tower::{Service, Layer};
use futures::future::BoxFuture;
use std::task::{Context, Poll};
use std::pin::Pin;

#[derive(Debug, Error)]
pub enum RateLimitError {
    #[error("Redis error: {0}")]
    RedisError(#[from] redis::RedisError),
    #[error("Rate limit exceeded")]
    LimitExceeded,
}

impl IntoResponse for RateLimitError {
    fn into_response(self) -> Response {
        match self {
            RateLimitError::RedisError(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
            RateLimitError::LimitExceeded => StatusCode::TOO_MANY_REQUESTS.into_response(),
        }
    }
}

pub struct RateLimiter {
    redis: Arc<RedisClient>,
    key_prefix: String,
    max_requests: usize,
    window_seconds: usize,
    logger: Logger,
}

impl RateLimiter {
    pub fn new(redis: Arc<RedisClient>, key_prefix: &str, max_requests: usize, window_seconds: usize, logger: Logger) -> Self {
        Self {
            redis,
            key_prefix: key_prefix.to_string(),
            max_requests,
            window_seconds,
            logger,
        }
    }

    pub async fn is_rate_limited(&self, key: &str) -> Result<bool, RateLimitError> {
        let mut conn = self.redis.get_connection().await?;
        let full_key = format!("{}:{}", self.key_prefix, key);

        let current: Option<usize> = conn.get(&full_key).await?;
        let current = current.unwrap_or(0);

        if current >= self.max_requests {
            info!(self.logger, "Rate limit exceeded"; "key" => key, "current" => current);
            Ok(true)
        } else {
            conn.incr(&full_key, 1).await?;
            conn.expire(&full_key, self.window_seconds).await?;
            Ok(false)
        }
    }
}

#[derive(Clone)]
pub struct RateLimitLayer {
    limiter: Arc<RateLimiter>,
    max_requests: usize,
    window_seconds: usize,
    key_extractor: Arc<dyn Fn(&Request<Body>) -> String + Send + Sync>,
}

impl RateLimitLayer {
    pub fn new<F>(limiter: Arc<RateLimiter>, max_requests: usize, window_seconds: usize, key_extractor: F) -> Self 
    where
        F: Fn(&Request<Body>) -> String + Send + Sync + 'static,
    {
        Self {
            limiter,
            max_requests,
            window_seconds,
            key_extractor: Arc::new(key_extractor),
        }
    }
}

impl<S> Layer<S> for RateLimitLayer {
    type Service = RateLimitService<S>;

    fn layer(&self, service: S) -> Self::Service {
        RateLimitService {
            inner: service,
            limiter: self.limiter.clone(),
            max_requests: self.max_requests,
            window_seconds: self.window_seconds,
            key_extractor: self.key_extractor.clone(),
        }
    }
}

#[derive(Clone)]
pub struct RateLimitService<S> {
    inner: S,
    limiter: Arc<RateLimiter>,
    max_requests: usize,
    window_seconds: usize,
    key_extractor: Arc<dyn Fn(&Request<Body>) -> String + Send + Sync>,
}

impl<S> Service<Request<Body>> for RateLimitService<S>
where
    S: Service<Request<Body>, Response = Response> + Send + 'static,
    S::Future: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        let limiter = self.limiter.clone();
        let key = (self.key_extractor)(&req);
        let fut = self.inner.call(req);

        Box::pin(async move {
            match limiter.is_rate_limited(&key).await {
                Ok(true) => {
                    Ok(RateLimitError::LimitExceeded.into_response())
                }
                Ok(false) => fut.await,
                Err(e) => {
                    error!(limiter.logger, "Rate limiting error"; "error" => %e);
                    fut.await
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        routing::get,
        Router,
    };
    use tower::ServiceExt;
    use hyper::StatusCode;

    #[tokio::test]
    async fn test_rate_limiter() {
        let redis_client = Arc::new(RedisClient::open("redis://127.0.0.1/").unwrap());
        let logger = Logger::root(slog::Discard.fuse(), slog::o!());
        
        let limiter = Arc::new(RateLimiter::new(
            redis_client.clone(),
            "test",
            2,
            60,
            logger.clone(),
        ));

        let layer = RateLimitLayer::new(
            limiter.clone(),
            2,
            60,
            |req: &Request<Body>| req.extensions().get::<String>().unwrap().clone(),
        );

        let app = Router::new()
            .route("/", get(|| async { "Hello, World!" }))
            .layer(layer);

        let mut mock_req = || {
            let mut req = Request::builder().uri("/").body(Body::empty()).unwrap();
            req.extensions_mut().insert("127.0.0.1".to_string());
            req
        };

        let response = app.clone().oneshot(mock_req()).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let response = app.clone().oneshot(mock_req()).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let response = app.oneshot(mock_req()).await.unwrap();
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    }
}