use redis::{Client as RedisClient, AsyncCommands};
use std::time::Duration;
use actix_web::{dev::ServiceRequest, Error, HttpResponse};
use actix_web::web::Data;
use futures::future::{Ready, ok, err};
use std::sync::Arc;

pub struct RateLimiter {
    redis: Arc<RedisClient>,
    key_prefix: String,
    max_requests: usize,
    window_seconds: usize,
}

impl RateLimiter {
    pub fn new(redis: Arc<RedisClient>, key_prefix: &str, max_requests: usize, window_seconds: usize) -> Self {
        Self {
            redis,
            key_prefix: key_prefix.to_string(),
            max_requests,
            window_seconds,
        }
    }

    pub async fn is_rate_limited(&self, key: &str) -> Result<bool, redis::RedisError> {
        let mut conn = self.redis.get_async_connection().await?;
        let full_key = format!("{}:{}", self.key_prefix, key);

        let current: Option<usize> = conn.get(&full_key).await?;
        let current = current.unwrap_or(0);

        if current >= self.max_requests {
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
}

impl RateLimitMiddleware {
    pub fn new(limiter: Arc<RateLimiter>, max_requests: usize, window_seconds: usize) -> Self {
        Self {
            limiter,
            max_requests,
            window_seconds,
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
        })
    }
}

pub struct RateLimitMiddlewareService<S> {
    service: S,
    limiter: Arc<RateLimiter>,
    max_requests: usize,
    window_seconds: usize,
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
        let fut = self.service.call(req);

        Box::pin(async move {
            let key = "global"; // You might want to use a more specific key, e.g., based on user ID or IP
            match limiter.is_rate_limited(key).await {
                Ok(true) => {
                    let response = HttpResponse::TooManyRequests()
                        .insert_header(("X-RateLimit-Limit", max_requests.to_string()))
                        .insert_header(("X-RateLimit-Window-Seconds", window_seconds.to_string()))
                        .finish();
                    Ok(req.into_response(response))
                }
                Ok(false) => fut.await,
                Err(_) => {
                    // If there's an error with rate limiting, we'll allow the request to proceed
                    fut.await
                }
            }
        })
    }
}