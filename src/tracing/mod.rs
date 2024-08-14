use actix_web::dev::{Service, ServiceRequest, ServiceResponse, Transform};
use actix_web::Error;
use futures::future::{ok, Ready};
use opentelemetry::trace::{Span, Tracer, TraceContextExt};
use opentelemetry::global;
use opentelemetry::KeyValue;
use std::pin::Pin;
use std::future::Future;
use std::time::Instant;

/// Tracing middleware for Actix-Web applications
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

impl<S, B> Transform<S, ServiceRequest> for TracingMiddleware
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type InitError = ();
    type Transform = TracingMiddlewareService<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ok(TracingMiddlewareService { 
            service,
            service_name: self.service_name.clone(),
        })
    }
}

pub struct TracingMiddlewareService<S> {
    service: S,
    service_name: String,
}

impl<S, B> Service<ServiceRequest> for TracingMiddlewareService<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;

    actix_web::dev::forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let tracer = global::tracer(&self.service_name);
        let start_time = Instant::now();

        let mut span = tracer.start(format!("{} {}", req.method(), req.path()));
        span.set_attribute(KeyValue::new("http.method", req.method().as_str().to_owned()));
        span.set_attribute(KeyValue::new("http.route", req.path().to_owned()));
        
        if let Some(user_agent) = req.headers().get("User-Agent") {
            if let Ok(user_agent) = user_agent.to_str() {
                span.set_attribute(KeyValue::new("http.user_agent", user_agent.to_owned()));
            }
        }

        let fut = self.service.call(req);

        Box::pin(async move {
            let res = fut.await;
            let elapsed = start_time.elapsed();

            match &res {
                Ok(response) => {
                    span.set_attribute(KeyValue::new("http.status_code", response.status().as_u16() as i64));
                }
                Err(error) => {
                    span.set_attribute(KeyValue::new("error", error.to_string()));
                }
            }

            span.set_attribute(KeyValue::new("duration_ms", elapsed.as_millis() as i64));
            span.end();

            res
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{test, web, App, HttpResponse};
    use opentelemetry::sdk::trace::{self, Tracer};

    #[actix_web::test]
    async fn test_tracing_middleware() {
        let tracer = Tracer::builder()
            .with_sampler(trace::Sampler::AlwaysOn)
            .build();
        let _guard = global::set_tracer_provider(tracer);

        let app = test::init_service(
            App::new()
                .wrap(TracingMiddleware::new("test-service"))
                .route("/", web::get().to(|| HttpResponse::Ok().body("Hello world!")))
        ).await;

        let req = test::TestRequest::get().uri("/").to_request();
        let resp = test::call_service(&app, req).await;

        assert!(resp.status().is_success());
    }
}