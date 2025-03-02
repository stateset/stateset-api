use axum::{
    body::Body,
    http::{Request, Response, StatusCode},
    response::IntoResponse,
};
use opentelemetry::{
    global,
    trace::{FutureExt, Span, TraceContextExt, Tracer, TracerProvider},
    Context, KeyValue,
};
use slog::{info, warn, Logger};
use std::sync::Arc;
use std::time::Instant;
use thiserror::Error;
use tower::{Layer, Service};
use futures::future::BoxFuture;
use std::task::{Context as TaskContext, Poll};
use pin_project_lite::pin_project;

/// Error type for tracing middleware operations
#[derive(Error, Debug)]
pub enum TracingError {
    #[error("Tracer initialization failed: {0}")]
    TracerInit(String),
    #[error("Span creation failed: {0}")]
    SpanCreation(String),
}

/// Configuration for tracing middleware
#[derive(Clone)]
pub struct TracingConfig {
    service_name: String,
    tracer_provider: Arc<dyn TracerProvider + Send + Sync>,
}

/// Tracing middleware layer
#[derive(Clone)]
pub struct TracingMiddleware {
    config: TracingConfig,
    logger: Logger,
}

impl TracingMiddleware {
    /// Creates a new tracing middleware instance
    pub fn new(
        service_name: impl Into<String>,
        logger: Logger,
        tracer_provider: impl TracerProvider + Send + Sync + 'static,
    ) -> Self {
        Self {
            config: TracingConfig {
                service_name: service_name.into(),
                tracer_provider: Arc::new(tracer_provider),
            },
            logger,
        }
    }
}

impl<S> Layer<S> for TracingMiddleware {
    type Service = TracingMiddlewareService<S>;

    fn layer(&self, service: S) -> Self::Service {
        TracingMiddlewareService {
            service,
            config: self.config.clone(),
            logger: self.logger.clone(),
        }
    }
}

/// Service wrapper that adds tracing capabilities
#[derive(Clone)]
pub struct TracingMiddlewareService<S> {
    service: S,
    config: TracingConfig,
    logger: Logger,
}

impl<S> Service<Request<Body>> for TracingMiddlewareService<S>
where
    S: Service<Request<Body>, Response = Response<Body>> + Send + Clone + 'static,
    S::Future: Send + 'static,
    S::Error: std::fmt::Display + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut TaskContext<'_>) -> Poll<Result<(), Self::Error>> {
        self.service.poll_ready(cx)
    }

    fn call(&mut self, mut req: Request<Body>) -> Self::Future {
        let tracer = self.config.tracer_provider.tracer(&self.config.service_name);
        let mut span = tracer.start(format!("{} {}", req.method(), req.uri().path()));
        let start_time = Instant::now();

        span.set_attribute(KeyValue::new("http.method", req.method().to_string()));
        span.set_attribute(KeyValue::new("http.route", req.uri().path().to_string()));
        
        if let Some(ua) = req.headers().get("User-Agent") {
            if let Ok(ua_str) = ua.to_str() {
                span.set_attribute(KeyValue::new("http.user_agent", ua_str.to_string()));
            }
        }

        let cx = Context::current_with_span(span);
        req.extensions_mut().insert(cx.clone());
        
        let logger = self.logger.clone();
        let service = self.service.clone();
        
        Box::pin(async move {
            let response = service.call(req).with_context(cx.clone()).await;
            
            let span = cx.span();
            let duration = start_time.elapsed();
            
            match &response {
                Ok(resp) => {
                    span.set_attribute(KeyValue::new("http.status_code", resp.status().as_u16() as i64));
                    info!(logger, "Request completed";
                        "method" => resp.status().as_str(),
                        "path" => span.attribute_value("http.route").unwrap_or_default().as_str(),
                        "duration_ms" => duration.as_millis()
                    );
                }
                Err(error) => {
                    span.set_attribute(KeyValue::new("error", error.to_string()));
                    span.record_exception(&format!("Request failed: {}", error));
                    warn!(logger, "Request failed"; "error" => %error, "duration_ms" => duration.as_millis());
                }
            }
            
            span.set_attribute(KeyValue::new("duration_ms", duration.as_millis() as i64));
            span.end();
            
            response
        })
    }
}

/// Helper function to setup tracing with default configuration
pub fn setup_default_tracing(service_name: &str) -> Result<(), TracingError> {
    let tracer_provider = opentelemetry_sdk::trace::TracerProvider::builder()
        .with_config(opentelemetry_sdk::trace::Config::default()
            .with_resource(opentelemetry_sdk::Resource::new(vec![
                KeyValue::new("service.name", service_name.to_string())
            ])))
        .with_simple_processor(
            opentelemetry_stdout::SpanExporter::builder()
                .build()
        )
        .build();

    global::set_tracer_provider(tracer_provider);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{routing::get, Router};
    use opentelemetry_sdk::trace::{TracerProvider as SdkTracerProvider, Config as TraceConfig};
    use opentelemetry_sdk::Resource;
    use slog::{o, Drain, Logger};
    use slog_term::{TermDecorator, CompactFormat};
    use slog_async::Async;
    use tower::ServiceExt;

    fn setup_test_logger() -> Logger {
        let decorator = TermDecorator::new().build();
        let drain = CompactFormat::new(decorator).build().fuse();
        let drain = Async::new(drain).build().fuse();
        Logger::root(drain, o!())
    }

    fn setup_test_tracer_provider() -> SdkTracerProvider {
        SdkTracerProvider::builder()
            .with_config(TraceConfig::default()
                .with_resource(Resource::new(vec![
                    KeyValue::new("service.name", "test-service")
                ])))
            .with_simple_processor(
                opentelemetry_stdout::SpanExporter::builder()
                    .build()
            )
            .build()
    }

    async fn test_handler() -> impl IntoResponse {
        "Hello, World!"
    }

    #[tokio::test]
    async fn test_tracing_middleware() {
        let logger = setup_test_logger();
        let tracer_provider = setup_test_tracer_provider();
        global::set_tracer_provider(tracer_provider.clone());

        let middleware = TracingMiddleware::new("test-service", logger, tracer_provider);
        let app = Router::new()
            .route("/", get(test_handler))
            .layer(middleware);

        let request = Request::builder()
            .uri("/")
            .header("User-Agent", "TestAgent/1.0")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_tracing_with_error() {
        let logger = setup_test_logger();
        let tracer_provider = setup_test_tracer_provider();
        global::set_tracer_provider(tracer_provider.clone());

        async fn error_handler() -> Result<String, String> {
            Err("Test error".to_string())
        }

        let middleware = TracingMiddleware::new("test-service", logger, tracer_provider);
        let app = Router::new()
            .route("/", get(error_handler))
            .layer(middleware);

        let request = Request::builder()
            .uri("/")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }
}