use axum::{
    body::Body,
    http::{Request, Response},
    response::IntoResponse,
};
use futures::future::BoxFuture;
use opentelemetry::trace::{Span, Tracer, TraceContextExt};
use opentelemetry::global;
use opentelemetry::KeyValue;
use std::time::Instant;
use tower::{Layer, Service};
use pin_project_lite::pin_project;
use std::task::{Context, Poll};
use std::pin::Pin;
use std::future::Future;

#[derive(Clone)]
pub struct TracingMiddleware {
    service_name: String,
}

impl TracingMiddleware {
    pub fn new(service_name: impl Into<String>) -> Self {
        TracingMiddleware {
            service_name: service_name.into(),
        }
    }
}

impl<S> Layer<S> for TracingMiddleware {
    type Service = TracingMiddlewareService<S>;

    fn layer(&self, service: S) -> Self::Service {
        TracingMiddlewareService {
            service,
            service_name: self.service_name.clone(),
        }
    }
}

#[derive(Clone)]
pub struct TracingMiddlewareService<S> {
    service: S,
    service_name: String,
}

impl<S> Service<Request<Body>> for TracingMiddlewareService<S>
where
    S: Service<Request<Body>, Response = Response> + Send + 'static,
    S::Future: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = TracingFuture<S::Future>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.service.poll_ready(cx)
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        let tracer = global::tracer(&self.service_name);
        let start_time = Instant::now();

        let mut span = tracer.start(format!("{} {}", req.method(), req.uri().path()));
        span.set_attribute(KeyValue::new("http.method", req.method().as_str().to_owned()));
        span.set_attribute(KeyValue::new("http.route", req.uri().path().to_owned()));
        
        if let Some(user_agent) = req.headers().get("User-Agent") {
            if let Ok(user_agent) = user_agent.to_str() {
                span.set_attribute(KeyValue::new("http.user_agent", user_agent.to_owned()));
            }
        }

        let future = self.service.call(req);

        TracingFuture {
            future,
            span,
            start_time,
        }
    }
}

pin_project! {
    pub struct TracingFuture<F> {
        #[pin]
        future: F,
        span: Span,
        start_time: Instant,
    }
}

impl<F, E> Future for TracingFuture<F>
where
    F: Future<Output = Result<Response, E>>,
    E: std::fmt::Display,
{
    type Output = Result<Response, E>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        let result = match this.future.poll(cx) {
            Poll::Ready(result) => result,
            Poll::Pending => return Poll::Pending,
        };

        let elapsed = this.start_time.elapsed();

        match &result {
            Ok(response) => {
                this.span.set_attribute(KeyValue::new("http.status_code", response.status().as_u16() as i64));
            }
            Err(error) => {
                this.span.set_attribute(KeyValue::new("error", error.to_string()));
            }
        }

        this.span.set_attribute(KeyValue::new("duration_ms", elapsed.as_millis() as i64));
        this.span.end();

        Poll::Ready(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{routing::get, Router};
    use hyper::StatusCode;
    use opentelemetry::sdk::trace::{self, Tracer};
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_tracing_middleware() {
        let tracer = Tracer::builder()
            .with_sampler(trace::Sampler::AlwaysOn)
            .build();
        let _guard = global::set_tracer_provider(tracer);

        let app = Router::new()
            .route("/", get(|| async { "Hello, World!" }))
            .layer(TracingMiddleware::new("test-service"));

        let response = app
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }
}