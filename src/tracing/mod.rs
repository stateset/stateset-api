use axum::{
    body::Body,
    http::{Request, Response},
    response::IntoResponse,
};
use futures::future::BoxFuture;
use opentelemetry::{
    global,
    trace::{Span, TraceContextExt, Tracer},
    Context,
};
use opentelemetry::KeyValue;
use std::sync::Arc;
use std::time::Instant;
use tower::{Layer, Service};
use pin_project_lite::pin_project;
use std::task::{Context as TaskContext, Poll};
use std::pin::Pin;
use std::future::Future;
use slog::{info, Logger};
use thiserror::Error;

/// Custom error type for tracing middleware
#[derive(Error, Debug)]
pub enum TracingError {
    #[error("OpenTelemetry error: {0}")]
    OpenTelemetryError(String),
}

/// TracingMiddleware is a Tower Layer that adds tracing spans to incoming requests
#[derive(Clone)]
pub struct TracingMiddleware {
    service_name: String,
    logger: Logger,
}

impl TracingMiddleware {
    /// Creates a new TracingMiddleware instance
    pub fn new(service_name: impl Into<String>, logger: Logger) -> Self {
        Self {
            service_name: service_name.into(),
            logger,
        }
    }
}

impl<S> Layer<S> for TracingMiddleware {
    type Service = TracingMiddlewareService<S>;

    fn layer(&self, service: S) -> Self::Service {
        TracingMiddlewareService {
            service,
            service_name: self.service_name.clone(),
            logger: self.logger.clone(),
        }
    }
}

/// TracingMiddlewareService wraps the inner service and manages tracing spans
#[derive(Clone)]
pub struct TracingMiddlewareService<S> {
    service: S,
    service_name: String,
    logger: Logger,
}

impl<S> Service<Request<Body>> for TracingMiddlewareService<S>
where
    S: Service<Request<Body>, Response = Response> + Clone + Send + 'static,
    S::Future: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = TracingFuture<S::Future>;

    fn poll_ready(&mut self, cx: &mut TaskContext<'_>) -> Poll<Result<(), Self::Error>> {
        self.service.poll_ready(cx)
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        // Start tracing span
        let tracer = global::tracer(&self.service_name);
        let span = tracer.start(format!("{} {}", req.method(), req.uri().path()));
        let start_time = Instant::now();

        // Inject span context into the request's extensions for downstream use
        let cx = Context::current_with_span(span);
        let req = req.map(|body| {
            // Attach the current context to the request
            body
        });
        // Start the span with context
        let span = cx.span();
        span.set_attribute(KeyValue::new("http.method", req.method().as_str().to_owned()));
        span.set_attribute(KeyValue::new("http.route", req.uri().path().to_owned()));

        if let Some(user_agent) = req.headers().get("User-Agent") {
            if let Ok(user_agent) = user_agent.to_str() {
                span.set_attribute(KeyValue::new("http.user_agent", user_agent.to_owned()));
            }
        }

        // Call the inner service
        let future = self.service.call(req);

        TracingFuture {
            future,
            span,
            start_time,
            logger: self.logger.clone(),
        }
    }
}

pin_project! {
    pub struct TracingFuture<F> {
        #[pin]
        future: F,
        span: Span,
        start_time: Instant,
        logger: Logger,
    }
}

impl<F, E> Future for TracingFuture<F>
where
    F: Future<Output = Result<Response, E>>,
    E: std::fmt::Display,
{
    type Output = Result<Response, E>;

    fn poll(self: Pin<&mut Self>, cx: &mut TaskContext<'_>) -> Poll<Self::Output> {
        let this = self.project();
        match this.future.poll(cx) {
            Poll::Ready(result) => {
                let elapsed = this.start_time.elapsed();
                match &result {
                    Ok(response) => {
                        this.span.set_attribute(KeyValue::new("http.status_code", response.status().as_u16() as i64));
                    }
                    Err(error) => {
                        this.span.set_attribute(KeyValue::new("error", error.to_string()));
                        this.span.record_exception(&opentelemetry::trace::Exception::new(
                            error.to_string(),
                            None,
                        ));
                    }
                }
                this.span.set_attribute(KeyValue::new("duration_ms", elapsed.as_millis() as i64));
                this.span.end();
                Poll::Ready(result)
            }
            Poll::Pending => Poll::Pending,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{routing::get, Router};
    use hyper::StatusCode;
    use opentelemetry::sdk::trace::{self, TracerProvider};
    use opentelemetry::runtime::Tokio;
    use opentelemetry::sdk::Resource;
    use opentelemetry::KeyValue;
    use opentelemetry::trace::Tracer;
    use opentelemetry::global;
    use slog::{Drain, Logger};
    use slog_async;
    use slog_term;
    use tower::ServiceExt; // for `oneshot`

    #[tokio::test]
    async fn test_tracing_middleware() {
        // Initialize a test tracer
        let tracer = trace::TracerProvider::builder()
            .with_simple_processor(trace::BatchSpanProcessor::builder(
                opentelemetry_otlp::ExporterConfig::default(),
            ).build())
            .with_resource(Resource::new(vec![KeyValue::new("service.name", "test-service")]))
            .build()
            .versioned_tracer(
                "test-service",
                Some(env!("CARGO_PKG_VERSION")),
                None,
            );

        global::set_tracer_provider(tracer.provider());

        // Initialize logger
        let decorator = slog_term::TermDecorator::new().build();
        let drain = slog_term::CompactFormat::new(decorator).build().fuse();
        let drain = slog_async::Async::new(drain).build().fuse();
        let logger = Logger::root(drain, slog::o!());

        // Define the tracing middleware
        let tracing_layer = TracingMiddleware::new("test-service", logger.clone());

        // Define a simple handler
        let app = Router::new()
            .route("/", get(|| async { "Hello, World!" }))
            .layer(tracing_layer);

        // Define a mock request
        let mock_req = || {
            let mut req = Request::builder()
                .uri("/")
                .header("User-Agent", "TestAgent/1.0")
                .body(Body::empty())
                .unwrap();
            req
        };

        // Send first request: should pass
        let response = app.clone().oneshot(mock_req()).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        // Send second request: should pass
        let response = app.clone().oneshot(mock_req()).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        // Send third request: should pass (no rate limiting in this middleware)
        let response = app.oneshot(mock_req()).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }
}
