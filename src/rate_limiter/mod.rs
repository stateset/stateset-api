use redis::aio::ConnectionManager;

pub struct RateLimiter {
    redis: Arc<ConnectionManager>,
    key_prefix: String,
    max_requests: usize,
    window_seconds: usize,
    logger: Logger,
}

impl RateLimiter {
    pub async fn new(redis_url: &str, key_prefix: &str, max_requests: usize, window_seconds: usize, logger: Logger) -> Result<Self, redis::RedisError> {
        let client = redis::Client::open(redis_url)?;
        let conn_manager = ConnectionManager::new(client).await?;
        
        Ok(Self {
            redis: Arc::new(conn_manager),
            key_prefix: key_prefix.to_string(),
            max_requests,
            window_seconds,
            logger,
        })
    }

    pub async fn is_rate_limited(&self, key: &str) -> Result<bool, RateLimitError> {
        let mut conn = self.redis.clone();
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
    key_extractor: Arc<dyn Fn(&Request<Body>) -> String + Send + Sync>,
}

impl RateLimitLayer {
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
        let redis_url = "redis://127.0.0.1/";
        let logger = Logger::root(slog::Discard.fuse(), slog::o!());
        
        let limiter = Arc::new(RateLimiter::new(
            redis_url,
            "test",
            2,
            60,
            logger.clone(),
        ).await.unwrap());

        let layer = RateLimitLayer::new(
            limiter.clone(),
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