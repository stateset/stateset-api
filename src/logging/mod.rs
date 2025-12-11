use axum::{
    body::Body,
    http::{Request, StatusCode},
    middleware::Next,
    response::Response,
    Router,
};
use slog::{o, Drain, Logger};
use slog_async::Async;
use slog_term::{FullFormat, TermDecorator};
use std::sync::Arc;
use std::time::Instant;

/// Configuration for setting up the logger
#[derive(Debug, Clone)]
pub struct LoggerConfig {
    async_buffer_size: usize,
    use_color: bool,
}

impl Default for LoggerConfig {
    fn default() -> Self {
        Self {
            async_buffer_size: 1024,
            use_color: true,
        }
    }
}

/// Sets up a logger with configurable options
pub fn setup_logger(config: LoggerConfig) -> Logger {
    let decorator = {
        let builder = TermDecorator::new();
        let builder = if config.use_color {
            builder.force_color()
        } else {
            builder
        };
        builder.build()
    };

    let drain = FullFormat::new(decorator).build().fuse();

    let drain = Async::new(drain)
        .chan_size(config.async_buffer_size)
        .build()
        .fuse();

    Logger::root(drain, o!("version" => env!("CARGO_PKG_VERSION")))
}

/// State struct for logging middleware
#[derive(Clone)]
pub struct LoggingState {
    logger: Logger,
}

impl LoggingState {
    pub fn new(logger: Logger) -> Self {
        Self { logger }
    }
}

/// Logging middleware for Axum applications
pub async fn logging_middleware(
    axum::extract::State(state): axum::extract::State<Arc<LoggingState>>,
    req: Request<Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    let start_time = Instant::now();
    let method = req.method().to_string();
    let path = req.uri().path().to_string();

    let response = next.run(req).await;
    let duration = start_time.elapsed();
    let status = response.status().as_u16();
    let duration_ms: u128 = duration.as_millis();

    slog::info!(
        &state.logger,
        "HTTP request handled";
        "method" => method,
        "path" => path,
        "status" => status,
        "duration_ms" => duration_ms,
    );

    Ok(response)
}

/// Creates an Axum application with logging middleware
pub fn create_app(logger: Logger) -> Router<Arc<LoggingState>> {
    let logging_state = Arc::new(LoggingState::new(logger));

    Router::new()
        .route("/health", axum::routing::get(|| async { "OK" }))
        // Add more routes here
        .layer(axum::middleware::from_fn_with_state(
            logging_state.clone(),
            logging_middleware,
        ))
        .with_state(logging_state)
}

#[cfg(all(test, feature = "mock-tests"))]
mod tests {
    use super::*;
    use axum::{
        body::{to_bytes, Body},
        http::{Request, StatusCode},
        routing::get,
        Router, ServiceExt,
    };
    use tower::ServiceExt;

    async fn test_handler() -> &'static str {
        "Hello, World!"
    }

    #[tokio::test]
    async fn test_logging_middleware() {
        let config = LoggerConfig {
            async_buffer_size: 128, // Smaller buffer for tests
            use_color: false,       // No color in test output
        };

        let logger = setup_logger(config);
        let app = create_app(logger);

        let request = Request::builder()
            .uri("/health")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        // Test with the actual handler
        let app = Router::new()
            .route("/", get(test_handler))
            .merge(create_app(setup_logger(config)));

        let request = Request::builder().uri("/").body(Body::empty()).unwrap();

        let response = app.oneshot(request).await.unwrap();
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(body, "Hello, World!");
    }
}
