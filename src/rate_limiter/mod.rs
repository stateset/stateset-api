use redis::{Client as RedisClient, AsyncCommands};
use std::time::Duration;
use actix_web::{dev::ServiceRequest, Error, HttpResponse, ResponseError};
use actix_web::web::Data;
use futures::future::{Ready, ok, err};
use std::sync::Arc;
use slog::{Logger, info, error};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RateLimitError {
    #[error("Redis error: {0}")]
    RedisError(#[from] redis::RedisError),
    #[error("Rate limit exceeded")]
    LimitExceeded,
}

impl ResponseError for RateLimitError {
    fn error_response(&self) -> HttpResponse {
        match self {
            RateLimitError::RedisError(_) => HttpResponse::InternalServerError().finish(),
            RateLimitError::LimitExceeded => HttpResponse::TooManyRequests().finish(),
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
        let mut conn = self.redis.get_async_connection().await?;
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

pub struct RateLimitMiddleware {
    limiter: Arc<RateLimiter>,
    max_requests: usize,
    window_seconds: usize,
    key_extractor: Arc<dyn Fn(&ServiceRequest) -> String + Send + Sync>,
}

impl RateLimitMiddleware {
    pub fn new<F>(limiter: Arc<RateLimiter>, max_requests: usize, window_seconds: usize, key_extractor: F) -> Self 
    where
        F: Fn(&ServiceRequest) -> String + Send + Sync + 'static,
    {
        Self {
            limiter,
            max_requests,
            window_seconds,
            key_extractor: Arc::new(key_extractor),
        }
    }
}

impl<S> actix_web::dev::Transform<S, ServiceRequest> for RateLimitMiddleware
where
    S: actix_web::dev::Service<ServiceRequest, Response = actix_web::dev::ServiceResponse, Error = Error>,
    S::Future: 'static,
{
    type Response = actix_web::dev::ServiceResponse;
    type Error = Error;
    type InitError = ();
    type Transform = RateLimitMiddlewareService<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ok(RateLimitMiddlewareService {
            service,
            limiter: self.limiter.clone(),
            max_requests: self.max_requests,
            window_seconds: self.window_seconds,
            key_extractor: self.key_extractor.clone(),
        })
    }
}

pub struct RateLimitMiddlewareService<S> {
    service: S,
    limiter: Arc<RateLimiter>,
    max_requests: usize,
    window_seconds: usize,
    key_extractor: Arc<dyn Fn(&ServiceRequest) -> String + Send + Sync>,
}

impl<S> actix_web::dev::Service<ServiceRequest> for RateLimitMiddlewareService<S>
where
    S: actix_web::dev::Service<ServiceRequest, Response = actix_web::dev::ServiceResponse, Error = Error>,
    S::Future: 'static,
{
    type Response = actix_web::dev::ServiceResponse;
    type Error = Error;
    type Future = std::pin::Pin<Box<dyn std::future::Future<Output = Result<Self::Response, Self::Error>>>>;

    actix_web::dev::forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let limiter = self.limiter.clone();
        let max_requests = self.max_requests;
        let window_seconds = self.window_seconds;
        let key = (self.key_extractor)(&req);
        let fut = self.service.call(req);

        Box::pin(async move {
            match limiter.is_rate_limited(&key).await {
                Ok(true) => {
                    Err(RateLimitError::LimitExceeded.into())
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
    use actix_web::{test, web, App, HttpResponse};
    use slog::{Drain, Logger};

    #[actix_web::test]
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

        let middleware = RateLimitMiddleware::new(
            limiter.clone(),
            2,
            60,
            |req: &ServiceRequest| req.peer_addr().unwrap().ip().to_string(),
        );

        let app = test::init_service(
            App::new()
                .wrap(middleware)
                .route("/", web::get().to(|| HttpResponse::Ok().body("Hello world!")))
        ).await;

        let req = test::TestRequest::get().uri("/").to_request();
        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());

        let req = test::TestRequest::get().uri("/").to_request();
        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());

        let req = test::TestRequest::get().uri("/").to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 429);
    }
}