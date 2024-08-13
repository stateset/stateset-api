use slog::{Drain, Logger};
use slog_async::Async;
use slog_term::{FullFormat, TermDecorator};

pub fn setup_logger() -> Logger {
    let decorator = TermDecorator::new().build();
    let drain = FullFormat::new(decorator).build().fuse();
    let drain = Async::new(drain).build().fuse();
    Logger::root(drain, slog::o!())
}

pub struct LoggingMiddleware {
    log: Logger,
}

impl LoggingMiddleware {
    pub fn new(log: Logger) -> Self {
        Self { log }
    }
}

impl<S, B> actix_web::dev::Transform<S, actix_web::dev::ServiceRequest> for LoggingMiddleware
where
    S: actix_web::dev::Service<actix_web::dev::ServiceRequest, Response = actix_web::dev::ServiceResponse<B>, Error = actix_web::Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = actix_web::dev::ServiceResponse<B>;
    type Error = actix_web::Error;
    type Transform = LoggingMiddlewareService<S>;
    type InitError = ();
    type Future = std::future::Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        std::future::ready(Ok(LoggingMiddlewareService { service, log: self.log.clone() }))
    }
}

pub struct LoggingMiddlewareService<S> {
    service: S,
    log: Logger,
}

impl<S, B> actix_web::dev::Service<actix_web::dev::ServiceRequest> for LoggingMiddlewareService<S>
where
    S: actix_web::dev::Service<actix_web::dev::ServiceRequest, Response = actix_web::dev::ServiceResponse<B>, Error = actix_web::Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = actix_web::dev::ServiceResponse<B>;
    type Error = actix_web::Error;
    type Future = Box<dyn std::future::Future<Output = Result<Self::Response, Self::Error>>>;

    actix_web::dev::forward_ready!(service);

    fn call(&self, req: actix_web::dev::ServiceRequest) -> Self::Future {
        let start_time = std::time::Instant::now();
        let log = self.log.clone();

        let fut = self.service.call(req);
        Box::pin(async move {
            let res = fut.await?;
            let duration = start_time.elapsed();
            slog::info!(log, "Request processed";
                "method" => res.request().method().as_str(),
                "path" => res.request().path(),
                "status" => res.status().as_u16(),
                "duration" => format!("{:?}", duration),
            );
            Ok(res)
        })
    }
}