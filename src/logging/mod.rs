use slog::{Drain, Logger};
use slog_async::Async;
use slog_term::{FullFormat, TermDecorator};
use axum::{
    middleware::Next,
    response::Response,
    http::Request,
};
use std::time::Instant;
use std::sync::Arc;

pub fn setup_logger() -> Logger {
    let decorator = TermDecorator::new().build();
    let drain = FullFormat::new(decorator).build().fuse();
    let drain = Async::new(drain).build().fuse();
    Logger::root(drain, slog::o!())
}

#[derive(Clone)]
pub struct LoggingState {
    log: Logger,
}

impl LoggingState {
    pub fn new(log: Logger) -> Self {
        Self { log }
    }
}

pub async fn logging_middleware<B>(
    state: axum::extract::State<Arc<LoggingState>>,
    req: Request<B>,
    next: Next<B>,
) -> Response {
    let start_time = Instant::now();
    let method = req.method().clone();
    let uri = req.uri().clone();

    let response = next.run(req).await;

    let duration = start_time.elapsed();
    slog::info!(state.log, "Request processed";
        "method" => method.as_str(),
        "path" => uri.path(),
        "status" => response.status().as_u16(),
        "duration" => format!("{:?}", duration),
    );

    response
}

// Example of how to use the middleware in your Axum application
pub fn create_app(logger: Logger) -> axum::Router {
    let logging_state = Arc::new(LoggingState::new(logger));

    axum::Router::new()
        // ... your routes here ...
        .layer(axum::middleware::from_fn_with_state(logging_state.clone(), logging_middleware))
        .with_state(logging_state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        routing::get,
        Router,
    };
    use http::{Request, StatusCode};
    use tower::ServiceExt;

    async fn test_handler() -> &'static str {
        "Hello, World!"
    }

    #[tokio::test]
    async fn test_logging_middleware() {
        let logger = setup_logger();
        let logging_state = Arc::new(LoggingState::new(logger));

        let app = Router::new()
            .route("/", get(test_handler))
            .layer(axum::middleware::from_fn_with_state(logging_state.clone(), logging_middleware))
            .with_state(logging_state);

        let request = Request::builder()
            .uri("/")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        // Note: We can't easily check the log output in this test,
        // but we can verify that the middleware didn't interfere with the response.
    }
}