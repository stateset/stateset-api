use axum::{
    body::Body,
    http::{Request, StatusCode},
    response::IntoResponse,
};
use redis::AsyncCommands;
use slog::{info, Logger};
use std::{
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use thiserror::Error;
use tokio::sync::Mutex;
use tower::{Layer, Service};

// Define a custom error type for rate limiting
#[derive(Error, Debug)]
pub enum RateLimitError {
    #[error("Redis error: {0}")]
    RedisError(#[from] redis::RedisError),

    #[error("Rate limit exceeded")]
    RateLimitExceeded,
}

// RateLimiter struct encapsulating Redis connection and rate limiting parameters
pub struct RateLimiter {
    redis: Arc<redis::aio::ConnectionManager>,
    key_prefix: String,
    max_requests: usize,
    window_seconds: usize,
    logger: Logger,
    // To prevent multiple Lua scripts from running simultaneously for the same key
    locks: Arc<Mutex<std::collections::HashMap<String, ()>>>,
}

impl RateLimiter {
    /// Creates a new RateLimiter instance.
    pub async fn new(
        redis_url: &str,
        key_prefix: &str,
        max_requests: usize,
        window_seconds: usize,
        logger: Logger,
    ) -> Result<Self, redis::RedisError> {
        let client = redis::Client::open(redis_url)?;
        let conn_manager = redis::aio::ConnectionManager::new(client).await?;

        Ok(Self {
            redis: Arc::new(conn_manager),
            key_prefix: key_prefix.to_string(),
            max_requests,
            window_seconds,
            logger,
            locks: Arc::new(Mutex::new(std::collections::HashMap::new())),
        })
    }

    /// Checks if the given key has exceeded the rate limit.
    pub async fn is_rate_limited(&self, key: &str) -> Result<bool, RateLimitError> {
        let full_key = format!("{}:{}", self.key_prefix, key);
        let redis = self.redis.clone();

        // Acquire a lock for the specific key to prevent race conditions
        {
            let mut locks = self.locks.lock().await;
            if locks.contains_key(&full_key) {
                // Another request is processing this key, consider rate limited to prevent excessive load
                info!(
                    self.logger,
                    "Concurrent rate limit check for key"; "key" => &full_key
                );
                return Ok(true);
            }
            locks.insert(full_key.clone(), ());
        }

        // Define the Lua script for atomic INCR and EXPIRE
        let lua_script = r#"
            local current
            current = redis.call("INCR", KEYS[1])
            if tonumber(current) == 1 then
                redis.call("EXPIRE", KEYS[1], ARGV[1])
            end
            return current
        "#;

        // Execute the Lua script
        let current: usize = redis
            .eval(lua_script, &[&full_key], &[self.window_seconds.to_string()])
            .await?;

        // Release the lock
        {
            let mut locks = self.locks.lock().await;
            locks.remove(&full_key);
        }

        if current > self.max_requests {
            info!(
                self.logger,
                "Rate limit exceeded"; "key" => key, "current" => current
            );
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

// Implement Clone for RateLimiter to allow sharing across threads
impl Clone for RateLimiter {
    fn clone(&self) -> Self {
        Self {
            redis: Arc::clone(&self.redis),
            key_prefix: self.key_prefix.clone(),
            max_requests: self.max_requests,
            window_seconds: self.window_seconds,
            logger: self.logger.clone(),
            locks: Arc::clone(&self.locks),
        }
    }
}

/// Middleware layer for rate limiting
pub struct RateLimitLayer {
    limiter: Arc<RateLimiter>,
    key_extractor: Arc<dyn Fn(&Request<Body>) -> String + Send + Sync>,
}

impl RateLimitLayer {
    /// Creates a new RateLimitLayer
    pub fn new<F>(limiter: Arc<RateLimiter>, key_extractor: F) -> Self
    where
        F: Fn(&Request<Body>) -> String + Send + Sync + 'static,
    {
        Self {
            limiter,
            key_extractor: Arc::new(key_extractor),
        }
    }
}

impl<S> Layer<S> for RateLimitLayer {
    type Service = RateLimitMiddleware<S>;

    fn layer(&self, inner: S) -> Self::Service {
        RateLimitMiddleware {
            inner,
            limiter: Arc::clone(&self.limiter),
            key_extractor: Arc::clone(&self.key_extractor),
        }
    }
}

/// Middleware service that enforces rate limiting
pub struct RateLimitMiddleware<S> {
    inner: S,
    limiter: Arc<RateLimiter>,
    key_extractor: Arc<dyn Fn(&Request<Body>) -> String + Send + Sync>,
}

impl<S, B> Service<Request<B>> for RateLimitMiddleware<S>
where
    S: Service<Request<B>, Response = axum::response::Response> + Send + 'static,
    S::Future: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = tokio::task::JoinHandle<Result<Self::Response, Self::Error>>;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request<B>) -> Self::Future {
        let limiter = Arc::clone(&self.limiter);
        let key_extractor = Arc::clone(&self.key_extractor);
        let mut inner = self.inner.call(req);

        tokio::spawn(async move {
            let req = inner.get_ref();
            let key = (key_extractor)(req);

            match limiter.is_rate_limited(&key).await {
                Ok(true) => {
                    // Rate limit exceeded
                    Ok(
                        (StatusCode::TOO_MANY_REQUESTS, "Too many requests")
                            .into_response(),
                    )
                }
                Ok(false) => {
                    // Proceed to the inner service
                    inner.await
                }
                Err(e) => {
                    // Handle rate limiter error, log and respond with 500
                    info!(
                        limiter.logger,
                        "Rate limiter error"; "error" => %e
                    );
                    Ok(
                        (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            "Internal Server Error",
                        )
                            .into_response(),
                    )
                }
            }
        })
    }
}
