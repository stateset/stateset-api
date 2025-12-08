use axum::{
    body::{Body, Bytes, HttpBody},
    http::{HeaderMap, Request, Response, StatusCode},
    BoxError,
};
use futures::{future::BoxFuture, Future, FutureExt, StreamExt};
use metrics::{counter, histogram};
use opentelemetry::{
    global,
    trace::{FutureExt as OtelFutureExt, Span, TraceContextExt, Tracer},
    Context as OtelContext, KeyValue,
};
use serde::Serialize;
use serde_json::{self, Value as JsonValue};
use std::convert::Infallible;
use std::{
    cell::RefCell,
    collections::HashMap,
    fmt,
    marker::Send,
    pin::Pin,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    task::{Context, Poll},
    time::{Duration, Instant},
};
use thiserror::Error;
use tokio::time::Instant as TokioInstant;
use tower::{Layer, Service};
use tower_http::{classify::StatusInRangeAsFailures, trace::TraceLayer};
use tracing::instrument;

// Re-export tracing macros for use in lib.rs
use http_body_util::BodyExt;
use tower_http::trace::{
    DefaultOnBodyChunk, DefaultOnEos, DefaultOnFailure, DefaultOnRequest, DefaultOnResponse,
    MakeSpan,
};
pub use tracing::{debug, error, info, trace, warn};
use uuid::Uuid;

/**
 * Tracing and Observability Module
 *
 * This module provides utilities for:
 * 1. Request/response logging with sampling and filtering
 * 2. Error tracking and context
 * 3. Performance metrics
 * 4. Distributed tracing with OpenTelemetry
 */

/// Error type for tracing middleware operations
#[derive(Error, Debug)]
pub enum TracingError {
    #[error("Tracer initialization failed: {0}")]
    TracerInit(String),
    #[error("Span creation failed: {0}")]
    SpanCreation(String),
    #[error("Middleware error: {0}")]
    Middleware(String),
}

/// Represents the types of errors that can occur at different parts of the system
#[derive(Debug)]
pub enum ErrorKind {
    /// Database-related errors
    Database,
    /// Network or IO-related errors
    IO,
    /// Authentication or authorization errors
    Auth,
    /// Validation or business rule errors
    Validation,
    /// Integration with external services
    External,
    /// Unexpected or system errors
    Internal,
}

impl ToString for ErrorKind {
    fn to_string(&self) -> String {
        match self {
            ErrorKind::Database => "database_error",
            ErrorKind::IO => "io_error",
            ErrorKind::Auth => "auth_error",
            ErrorKind::Validation => "validation_error",
            ErrorKind::External => "external_service_error",
            ErrorKind::Internal => "internal_error",
        }
        .to_string()
    }
}

/// Request ID tracking information
#[derive(Clone, Debug)]
pub struct RequestId(pub String);

impl Default for RequestId {
    fn default() -> Self {
        RequestId(Uuid::new_v4().to_string())
    }
}

impl RequestId {
    pub fn new(value: impl Into<String>) -> Self {
        RequestId(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

tokio::task_local! {
    static CURRENT_REQUEST_ID: RefCell<Option<RequestId>>;
}

pub async fn scope_request_id<Fut, R>(request_id: RequestId, future: Fut) -> R
where
    Fut: Future<Output = R>,
{
    CURRENT_REQUEST_ID
        .scope(RefCell::new(Some(request_id)), future)
        .await
}

pub fn current_request_id() -> Option<RequestId> {
    CURRENT_REQUEST_ID
        .try_with(|cell| cell.borrow().clone())
        .ok()
        .flatten()
}

#[derive(Clone, Default)]
pub struct RequestSpanMaker;

impl<B> MakeSpan<B> for RequestSpanMaker {
    fn make_span(&mut self, request: &Request<B>) -> tracing::Span {
        let method = request.method().clone();
        let uri = request.uri().clone();
        let request_id = request
            .extensions()
            .get::<RequestId>()
            .cloned()
            .or_else(|| {
                request
                    .headers()
                    .get("x-request-id")
                    .and_then(|v| v.to_str().ok())
                    .map(RequestId::new)
            })
            .unwrap_or_default();

        let span = tracing::info_span!(
            "http.request",
            request_id = %request_id.as_str(),
            method = %method,
            uri = %uri,
        );
        span
    }
}

impl fmt::Display for RequestId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Enhanced request context for tracing
#[derive(Debug, Clone)]
pub struct RequestContext {
    /// Unique identifier for this request
    pub request_id: RequestId,
    /// Requested path
    pub path: String,
    /// HTTP Method used
    pub method: String,
    /// Timestamp when request was received
    pub start_time: TokioInstant,
    /// User ID if authenticated
    pub user_id: Option<Uuid>,
    /// Session ID if available
    pub session_id: Option<String>,
    /// Client IP address
    pub client_ip: Option<String>,
    /// User agent
    pub user_agent: Option<String>,
    /// OpenTelemetry context
    pub otel_context: Option<OtelContext>,
}

impl Default for RequestContext {
    fn default() -> Self {
        Self {
            request_id: RequestId::default(),
            path: String::new(),
            method: String::new(),
            start_time: TokioInstant::now(),
            user_id: None,
            session_id: None,
            client_ip: None,
            user_agent: None,
            otel_context: None,
        }
    }
}

/// Request logging middleware
#[derive(Clone, Debug)]
pub struct RequestLogger {
    /// Metrics tracking
    request_count: Arc<AtomicU64>,
    /// Include or exclude paths from logging
    excluded_paths: Vec<String>,
    /// Log request bodies for supported content types
    log_request_body: bool,
    /// Log response bodies for supported content types
    log_response_body: bool,
    /// Request sampler rate (0.0-1.0)
    sampling_rate: f64,
    /// Maximum body size to log
    max_body_size: usize,
}

impl Default for RequestLogger {
    fn default() -> Self {
        Self {
            request_count: Arc::new(AtomicU64::new(0)),
            excluded_paths: vec!["/health".to_string(), "/metrics".to_string()],
            log_request_body: false,
            log_response_body: false,
            sampling_rate: 1.0,
            max_body_size: 10240, // 10KB
        }
    }
}

impl RequestLogger {
    /// Create a new request logger with custom options
    pub fn new() -> Self {
        Self::default()
    }

    /// Exclude specific paths from normal logging
    pub fn exclude_paths(mut self, paths: Vec<String>) -> Self {
        self.excluded_paths = paths;
        self
    }

    /// Enable request body logging for supported content types
    pub fn log_request_body(mut self, enabled: bool) -> Self {
        self.log_request_body = enabled;
        self
    }

    /// Enable response body logging for supported content types
    pub fn log_response_body(mut self, enabled: bool) -> Self {
        self.log_response_body = enabled;
        self
    }

    /// Set sampling rate for detailed logging (0.0-1.0)
    pub fn sampling_rate(mut self, rate: f64) -> Self {
        self.sampling_rate = rate.clamp(0.0, 1.0);
        self
    }

    /// Set maximum body size to log
    pub fn max_body_size(mut self, size: usize) -> Self {
        self.max_body_size = size;
        self
    }

    /// Check if a path should be excluded from detailed logging
    fn is_excluded(&self, path: &str) -> bool {
        self.excluded_paths.iter().any(|p| path.starts_with(p))
    }

    /// Check if a request should be sampled for detailed logging
    fn should_sample(&self) -> bool {
        if self.sampling_rate >= 1.0 {
            return true;
        }
        if self.sampling_rate <= 0.0 {
            return false;
        }
        rand::random::<f64>() <= self.sampling_rate
    }

    /// Extract client IP from request headers
    fn extract_client_ip(headers: &HeaderMap) -> Option<String> {
        headers
            .get("x-forwarded-for")
            .and_then(|h| h.to_str().ok())
            .map(|s| s.split(',').next().unwrap_or(s).trim().to_string())
            .or_else(|| {
                headers
                    .get("x-real-ip")
                    .and_then(|h| h.to_str().ok())
                    .map(|s| s.to_string())
            })
    }

    /// Extract user agent from request headers
    fn extract_user_agent(headers: &HeaderMap) -> Option<String> {
        headers
            .get("user-agent")
            .and_then(|h| h.to_str().ok())
            .map(|s| s.to_string())
    }

    /// Limit body content to avoid excessive logging
    fn limit_body(body: &str, max_len: usize) -> String {
        if body.len() <= max_len {
            body.to_string()
        } else {
            format!(
                "{}... [truncated {} bytes]",
                &body[..max_len],
                body.len() - max_len
            )
        }
    }

    /// Is this a JSON content type
    fn is_json_content(headers: &HeaderMap) -> bool {
        headers
            .get("content-type")
            .and_then(|h| h.to_str().ok())
            .map(|s| s.contains("application/json"))
            .unwrap_or(false)
    }
}

/// Request logger middleware layer
#[derive(Clone, Debug)]
pub struct RequestLoggerLayer {
    logger: RequestLogger,
}

impl RequestLoggerLayer {
    pub fn new() -> Self {
        Self {
            logger: RequestLogger::default(),
        }
    }

    pub fn with_config(logger: RequestLogger) -> Self {
        Self { logger }
    }
}

impl<S> Layer<S> for RequestLoggerLayer {
    type Service = RequestLoggerMiddleware<S>;

    fn layer(&self, service: S) -> Self::Service {
        RequestLoggerMiddleware {
            service,
            logger: self.logger.clone(),
        }
    }
}

/// Request logger middleware
#[derive(Clone)]
pub struct RequestLoggerMiddleware<S> {
    service: S,
    logger: RequestLogger,
}

impl<S, ReqBody, ResBody> Service<Request<ReqBody>> for RequestLoggerMiddleware<S>
where
    S: Service<Request<BufferedBody<ReqBody>>, Response = Response<ResBody>>
        + Clone
        + Send
        + 'static,
    S::Future: Send + 'static,
    S::Error: Into<BoxError> + Send + 'static,
    ReqBody: HttpBody + Unpin + Send + 'static,
    ReqBody::Data: Send + 'static,
    ReqBody::Error: Into<BoxError> + Send + 'static,
    ResBody: axum::body::HttpBody<Data = axum::body::Bytes> + Unpin + Send + 'static,
    <ResBody as axum::body::HttpBody>::Error: std::error::Error + Send + Sync + 'static,
{
    type Response = Response<Body>;
    type Error = Infallible;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        match self.service.poll_ready(cx) {
            Poll::Ready(Ok(())) => Poll::Ready(Ok(())),
            // Swallow readiness errors and continue; downstream call will handle and log
            Poll::Ready(Err(_)) => Poll::Ready(Ok(())),
            Poll::Pending => Poll::Pending,
        }
    }

    fn call(&mut self, request: Request<ReqBody>) -> Self::Future
    where
        <ResBody as HttpBody>::Error: std::error::Error + Send + Sync + 'static,
        ResBody: axum::body::HttpBody + Unpin,
    {
        // We need to clone the service because we're consuming self in the future below
        let mut service = self.service.clone();
        let logger = self.logger.clone();

        // Increment request counter
        let req_count = logger.request_count.fetch_add(1, Ordering::SeqCst);

        // Create request ID and context
        let request_id = format!("{}-{}", Uuid::new_v4(), req_count);
        let path = request.uri().path().to_string();
        let route_label = request
            .extensions()
            .get::<axum::extract::MatchedPath>()
            .map(|m| m.as_str().to_string())
            .unwrap_or_else(|| path.clone());
        let method = request.method().to_string();
        let start_time = TokioInstant::now();

        // Extract client info
        let client_ip = RequestLogger::extract_client_ip(request.headers());
        let user_agent = RequestLogger::extract_user_agent(request.headers());

        // Create tracer and span for OpenTelemetry
        let current_otel_ctx = OtelContext::current();
        let tracer = global::tracer("stateset-api");
        let mut span = tracer.start(format!("{} {}", method, path));

        // Add attributes to span
        span.set_attribute(KeyValue::new("http.method", method.clone()));
        span.set_attribute(KeyValue::new("http.route", path.clone()));
        span.set_attribute(KeyValue::new("request_id", request_id.clone()));

        if let Some(ip) = &client_ip {
            span.set_attribute(KeyValue::new("client.ip", ip.clone()));
        }

        if let Some(ua) = &user_agent {
            span.set_attribute(KeyValue::new("http.user_agent", ua.clone()));
        }

        // Create OpenTelemetry context with span
        let otel_ctx = OtelContext::current_with_span(span);

        // Store context for other middleware and handlers to access
        let context = RequestContext {
            request_id: RequestId(request_id.clone()),
            path: path.clone(),
            method: method.clone(),
            start_time,
            user_id: None,    // This will be set by auth middleware
            session_id: None, // This will be set by auth middleware
            client_ip: client_ip.clone(),
            user_agent: user_agent.clone(),
            otel_context: Some(otel_ctx.clone()),
        };

        // Initial span for request
        let is_excluded = logger.is_excluded(&path);
        let should_sample = logger.should_sample();

        let detailed_logging = !is_excluded && should_sample;

        if !is_excluded {
            // Log basic request info
            info!(
                request_id = %request_id,
                method = %method,
                path = %path,
                client_ip = client_ip.as_deref().unwrap_or("unknown"),
                user_agent = user_agent.as_deref().unwrap_or("unknown"),
                "Request started"
            );
        }

        // Clone headers for potential body logging
        let has_json_body = RequestLogger::is_json_content(request.headers());
        let should_log_body = logger.log_request_body && has_json_body && detailed_logging;

        // Create request with context for downstream handlers
        let mut req_with_context = request.map(move |body| {
            if should_log_body {
                // If we want to log the body, buffer it
                BufferedBody {
                    body,
                    buffer: Vec::new(),
                    logging_enabled: true,
                    max_buffer_size: logger.max_body_size,
                }
            } else {
                // Otherwise pass through
                BufferedBody {
                    body,
                    buffer: Vec::new(),
                    logging_enabled: false,
                    max_buffer_size: 0,
                }
            }
        });

        // Add request context to extensions
        req_with_context.extensions_mut().insert(context);
        req_with_context.extensions_mut().insert(otel_ctx.clone());
        // Respect incoming request id if provided
        if let Some(req_id_hdr) = req_with_context
            .headers()
            .get("x-request-id")
            .and_then(|v| v.to_str().ok())
        {
            // Attach to span
            let span = otel_ctx.span();
            span.set_attribute(KeyValue::new("request_id", req_id_hdr.to_string()));
        }

        // This future will process the request/response and handle logging
        async move {
            // Execute the service with OpenTelemetry context
            let result: Result<_, BoxError> = service
                .call(req_with_context)
                .with_context(otel_ctx.clone())
                .await
                .map_err(Into::into);

            let duration = start_time.elapsed();
            let span = otel_ctx.span();

            // Handle response
            let response = match result {
                Ok(mut response) => {
                    let status = response.status();
                    span.set_attribute(KeyValue::new("http.status_code", status.as_u16() as i64));

                    // Propagate request id header
                    let headers = response.headers_mut();
                    headers.insert(
                        "X-Request-Id",
                        http::HeaderValue::from_str(&request_id)
                            .unwrap_or(http::HeaderValue::from_static("unknown")),
                    );

                    let res_has_json_body = RequestLogger::is_json_content(response.headers());
                    let should_log_res_body =
                        logger.log_response_body && res_has_json_body && detailed_logging;

                    if !is_excluded {
                        // Log response
                        match status.as_u16() {
                            s if s < 400 => {
                                info!(
                                    request_id = %request_id,
                                    status = %status.as_u16(),
                                    duration_ms = %duration.as_millis(),
                                    "Request completed"
                                )
                            }
                            s if s < 500 => {
                                warn!(
                                    request_id = %request_id,
                                    status = %status.as_u16(),
                                    duration_ms = %duration.as_millis(),
                                    "Client error"
                                )
                            }
                            _ => {
                                error!(
                                    request_id = %request_id,
                                    status = %status.as_u16(),
                                    duration_ms = %duration.as_millis(),
                                    "Server error"
                                )
                            }
                        }
                    }

                    // Check for slow requests
                    if duration > Duration::from_millis(1000) {
                        warn!(
                            request_id = %request_id,
                            method = %method,
                            path = %path,
                            duration_ms = %duration.as_millis(),
                            "Slow request detected"
                        );
                        span.set_attribute(KeyValue::new("slow_request", true));
                    }

                    // Return response with body logging if needed
                    if should_log_res_body {
                        // Extract and log response body
                        let (parts, body) = response.into_parts();
                        let bytes = match http_body_util::BodyExt::collect(body).await {
                            Ok(collected) => collected.to_bytes(),
                            Err(e) => {
                                error!(
                                    request_id = %request_id,
                                    error = %e,
                                    duration_ms = %duration.as_millis(),
                                    "Failed to collect response body"
                                );
                                let body = serde_json::json!({
                                    "error": "Internal Server Error",
                                    "message": e.to_string(),
                                    "request_id": request_id
                                });
                                // Build error response; if Response::builder fails, return a minimal error
                                let body_bytes = serde_json::to_vec(&body).unwrap_or_default();
                                return match Response::builder()
                                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                                    .header("content-type", "application/json")
                                    .body(Body::from(body_bytes))
                                {
                                    Ok(resp) => Ok(resp),
                                    Err(_) => Ok(Response::new(Body::from("Internal Server Error"))),
                                };
                            }
                        };

                        // Potentially augment JSON error body with request_id
                        let mut out_bytes = bytes.to_vec();
                        let status = parts.status;
                        let is_json = parts
                            .headers
                            .get("content-type")
                            .and_then(|h| h.to_str().ok())
                            .map(|ct| ct.contains("application/json"))
                            .unwrap_or(false);
                        if status.as_u16() >= 400 && is_json {
                            if let Ok(mut val) = serde_json::from_slice::<JsonValue>(&out_bytes) {
                                if let JsonValue::Object(ref mut map) = val {
                                    if !map.contains_key("request_id") {
                                        map.insert(
                                            "request_id".to_string(),
                                            JsonValue::String(request_id.clone()),
                                        );
                                        if let Ok(new_bytes) = serde_json::to_vec(&val) {
                                            out_bytes = new_bytes;
                                        }
                                    }
                                }
                            }
                        }

                        let body_str = String::from_utf8_lossy(&out_bytes);
                        let limited_body =
                            RequestLogger::limit_body(&body_str, logger.max_body_size);

                        info!(
                            request_id = %request_id,
                            response_body = %limited_body,
                            "Response body"
                        );

                        let body = Body::from(out_bytes);
                        Ok::<Response<Body>, Infallible>(Response::from_parts(parts, body))
                    } else {
                        let (parts, body) = response.into_parts();
                        let bytes = match http_body_util::BodyExt::collect(body).await {
                            Ok(collected) => collected.to_bytes(),
                            Err(e) => {
                                error!(
                                    request_id = %request_id,
                                    error = %e,
                                    duration_ms = %duration.as_millis(),
                                    "Failed to collect response body"
                                );
                                let body = serde_json::json!({
                                    "error": "Internal Server Error",
                                    "message": e.to_string(),
                                    "request_id": request_id
                                });
                                // Build error response; if Response::builder fails, return a minimal error
                                let body_bytes = serde_json::to_vec(&body).unwrap_or_default();
                                return match Response::builder()
                                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                                    .header("content-type", "application/json")
                                    .body(Body::from(body_bytes))
                                {
                                    Ok(resp) => Ok(resp),
                                    Err(_) => Ok(Response::new(Body::from("Internal Server Error"))),
                                };
                            }
                        };
                        // Potentially augment JSON error body with request_id
                        let mut out_bytes = bytes.to_vec();
                        let status = parts.status;
                        let is_json = parts
                            .headers
                            .get("content-type")
                            .and_then(|h| h.to_str().ok())
                            .map(|ct| ct.contains("application/json"))
                            .unwrap_or(false);
                        if status.as_u16() >= 400 && is_json {
                            if let Ok(mut val) = serde_json::from_slice::<JsonValue>(&out_bytes) {
                                if let JsonValue::Object(ref mut map) = val {
                                    if !map.contains_key("request_id") {
                                        map.insert(
                                            "request_id".to_string(),
                                            JsonValue::String(request_id.clone()),
                                        );
                                        if let Ok(new_bytes) = serde_json::to_vec(&val) {
                                            out_bytes = new_bytes;
                                        }
                                    }
                                }
                            }
                        }

                        let body = Body::from(out_bytes);
                        Ok::<Response<Body>, Infallible>(Response::from_parts(parts, body))
                    }
                }
                Err(err) => {
                    // Log the error and produce a 500 response
                    error!(
                        request_id = %request_id,
                        error = %err,
                        duration_ms = %duration.as_millis(),
                        "Request failed"
                    );

                    // Emit failure metrics
                    let status_code = StatusCode::INTERNAL_SERVER_ERROR.as_u16();
                    let duration_ms = duration.as_secs_f64() * 1000.0;
                    counter!("http_requests_total",
                        1,
                        "method" => method.clone(),
                        "route" => route_label.clone(),
                        "status" => status_code.to_string(),
                    );
                    histogram!("http_request_duration_ms",
                        duration_ms,
                        "method" => method.clone(),
                        "route" => route_label.clone(),
                        "status" => status_code.to_string(),
                    );

                    let body = serde_json::json!({
                        "error": "Internal Server Error",
                        "message": err.to_string(),
                        "request_id": request_id
                    });

                    Ok(Response::builder()
                        .status(StatusCode::INTERNAL_SERVER_ERROR)
                        .header("content-type", "application/json")
                        .body(Body::from(serde_json::to_vec(&body).unwrap_or_default()))
                        .unwrap_or_else(|_| Response::new(Body::empty())))
                }
            }?;

            // Record metrics for the request
            let status_code = response.status().as_u16();
            let duration_ms = duration.as_secs_f64() * 1000.0;
            counter!("http_requests_total",
                1,
                "method" => method.clone(),
                "route" => route_label.clone(),
                "status" => status_code.to_string(),
            );
            histogram!("http_request_duration_ms",
                duration_ms,
                "method" => method.clone(),
                "route" => route_label.clone(),
                "status" => status_code.to_string(),
            );

            // Always return Ok with Infallible error type
            // Update custom registry as well
            let _ = {
                // Avoid panics if metrics module changes; just call softly
                #[allow(unused_imports)]
                use crate::metrics::APP_METRICS;
                APP_METRICS.record_request(duration);
            };
            Ok(response)
        }
        .boxed()
    }
}

/// Body type that can buffer its contents for logging
pub struct BufferedBody<B> {
    body: B,
    buffer: Vec<u8>,
    logging_enabled: bool,
    max_buffer_size: usize,
}

impl<B> HttpBody for BufferedBody<B>
where
    B: HttpBody<Data = Bytes> + Unpin,
    B::Error: Into<BoxError>,
{
    type Data = Bytes;
    type Error = BoxError;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<hyper::body::Frame<Self::Data>, Self::Error>>> {
        let this = self.as_mut().get_mut();

        match Pin::new(&mut this.body).poll_frame(cx) {
            Poll::Ready(Some(Ok(frame))) => {
                if let Some(data) = frame.data_ref() {
                    let bytes = data.clone();

                    // If logging is enabled, store a copy of the data
                    if this.logging_enabled && this.buffer.len() < this.max_buffer_size {
                        let remaining = this.max_buffer_size - this.buffer.len();
                        let to_copy = std::cmp::min(remaining, bytes.len());
                        this.buffer.extend_from_slice(&bytes[..to_copy]);
                    }
                }

                Poll::Ready(Some(Ok(frame)))
            }
            Poll::Ready(Some(Err(err))) => Poll::Ready(Some(Err(err.into()))),
            Poll::Ready(None) => {
                // If we've buffered data and logging is enabled, log it
                if this.logging_enabled && !this.buffer.is_empty() {
                    if let Ok(body_str) = std::str::from_utf8(&this.buffer) {
                        let limited_body = RequestLogger::limit_body(body_str, 4096);
                        debug!(request_body = %limited_body, "Request body");
                    }
                }

                Poll::Ready(None)
            }
            Poll::Pending => Poll::Pending,
        }
    }
}

/// Adapter to read HttpBody and convert to tokio streams
#[allow(dead_code)] // Used for streaming body responses
struct StreamReader<B> {
    body: B,
}

#[allow(dead_code)] // Constructor for StreamReader
impl<B> StreamReader<B> {
    fn new(body: B) -> Self {
        Self { body }
    }
}

impl<B> futures::Stream for StreamReader<B>
where
    B: HttpBody<Data = axum::body::Bytes> + Unpin + Send,
    B::Error: Into<BoxError> + Send + Sync,
{
    type Item = Result<Bytes, BoxError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match Pin::new(&mut self.body).poll_frame(cx) {
            Poll::Ready(Some(Ok(frame))) => {
                if let Ok(chunk) = frame.into_data() {
                    Poll::Ready(Some(Ok(chunk)))
                } else {
                    // It's a trailer, skip it
                    self.poll_next(cx)
                }
            }
            Poll::Ready(Some(Err(err))) => Poll::Ready(Some(Err(err.into()))),
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }
}

/// Helper function to setup tracing with default configuration
///
/// Note: OpenTelemetry tracer provider setup is handled via the main tracing configuration.
/// This function provides a no-op default for cases where tracing is not required.
pub fn setup_default_tracing(_service_name: &str) -> Result<(), TracingError> {
    // Tracing is configured via TracingConfig::init() for full OpenTelemetry support.
    // This function exists for API compatibility and returns Ok for minimal setups.
    Ok(())
}

/// Configure tracing for the application with tower-http
pub fn configure_http_tracing() -> tower_http::trace::TraceLayer<
    tower_http::classify::SharedClassifier<StatusInRangeAsFailures>,
    RequestSpanMaker,
    DefaultOnRequest,
    DefaultOnResponse,
    DefaultOnBodyChunk,
    DefaultOnEos,
    DefaultOnFailure,
> {
    let classifier =
        tower_http::classify::SharedClassifier::new(StatusInRangeAsFailures::new(500..=599));
    TraceLayer::new(classifier)
        .make_span_with(RequestSpanMaker::default())
        .on_request(DefaultOnRequest::default())
        .on_response(DefaultOnResponse::default())
        .on_body_chunk(DefaultOnBodyChunk::default())
        .on_eos(DefaultOnEos::default())
        .on_failure(DefaultOnFailure::default())
}

/// Log an error with context
///
/// This function is used to log errors with additional context
/// information that helps with debugging.
#[instrument(level = "error", skip(err))]
pub fn log_error<E: std::fmt::Display>(err: &E, kind: ErrorKind, context: Option<&str>) {
    match context {
        Some(ctx) => {
            error!(error_type = kind.to_string(), context = ctx, error = %err, "Error occurred")
        }
        None => error!(error_type = kind.to_string(), error = %err, "Error occurred"),
    }
}

/// Log slow requests
pub fn log_slow_request(context: &RequestContext, duration: Duration, threshold: Duration) {
    if duration > threshold {
        warn!(
            request_id = %context.request_id,
            path = %context.path,
            method = %context.method,
            duration_ms = %duration.as_millis(),
            threshold_ms = %threshold.as_millis(),
            "Slow request detected"
        );
    }
}

/// Performance metrics for an operation
#[derive(Debug, Clone, Serialize)]
pub struct OperationMetrics {
    /// Name of the operation
    pub operation: String,
    /// Duration in milliseconds
    pub duration_ms: u128,
    /// Timestamp when operation started
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// Success or failure status
    pub success: bool,
    /// Additional context tags
    pub tags: HashMap<String, String>,
}

/// Runs a task and logs performance metrics
pub async fn with_metrics<F, Fut, T, E>(
    operation_name: &str,
    tags: Option<HashMap<String, String>>,
    task: F,
) -> Result<T, E>
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = Result<T, E>>,
    E: std::fmt::Display,
{
    // Start timer
    let start = Instant::now();
    let timestamp = chrono::Utc::now();

    // Run the task
    let result = task().await;

    // Calculate elapsed time
    let elapsed = start.elapsed();

    // Build metrics
    let metrics = OperationMetrics {
        operation: operation_name.to_string(),
        duration_ms: elapsed.as_millis(),
        timestamp,
        success: result.is_ok(),
        tags: tags.unwrap_or_default(),
    };

    // Log metrics
    match &result {
        Ok(_) => {
            info!(
                operation = %metrics.operation,
                duration_ms = %metrics.duration_ms,
                tags = ?metrics.tags,
                "Operation completed successfully"
            );
        }
        Err(e) => {
            error!(
                operation = %metrics.operation,
                duration_ms = %metrics.duration_ms,
                error = %e,
                tags = ?metrics.tags,
                "Operation failed"
            );
        }
    }

    result
}

#[cfg(all(test, feature = "mock-tests"))]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::{routing::get, Router};
    use http::Method;
    use serde_json::json;
    use tokio::sync::oneshot;
    use tower::util::ServiceExt; // bring oneshot into scope

    async fn test_handler() -> impl IntoResponse {
        "Hello, World!"
    }

    async fn json_handler() -> impl IntoResponse {
        Json(json!({"message": "Hello, World!"}))
    }

    #[derive(Serialize)]
    struct Json<T>(T);

    impl<T> IntoResponse for Json<T>
    where
        T: Serialize,
    {
        fn into_response(self) -> Response<Body> {
            match serde_json::to_vec(&self.0) {
                Ok(bytes) => {
                    Response::builder()
                        .status(StatusCode::OK)
                        .header("content-type", "application/json")
                        .body(Body::from(bytes))
                        .unwrap_or_else(|_| Response::new(Body::from("Serialization Error")))
                }
                Err(_) => {
                    Response::builder()
                        .status(StatusCode::INTERNAL_SERVER_ERROR)
                        .body(Body::empty())
                        .unwrap_or_else(|_| Response::new(Body::from("Internal Server Error")))
                }
            }
        }
    }

    #[tokio::test]
    async fn test_request_logger_middleware() {
        // Configure the logger
        let logger = RequestLogger::default()
            .log_request_body(true)
            .log_response_body(true);

        // Create a test app with our middleware
        let app = Router::new()
            .route("/", get(test_handler))
            .route("/json", get(json_handler))
            .layer(RequestLoggerLayer::with_config(logger));

        // Test a simple request
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/")
                    .method(Method::GET)
                    .header("user-agent", "test-agent")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        // Test a JSON request
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/json")
                    .method(Method::GET)
                    .header("user-agent", "test-agent")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{\"test\":\"value\"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get("content-type").unwrap(),
            "application/json"
        );
    }

    #[tokio::test]
    async fn test_with_metrics() {
        // Test successful operation
        let result: Result<i32, String> = with_metrics("test_operation", None, || async {
            tokio::time::sleep(Duration::from_millis(10)).await;
            Ok(42)
        })
        .await;

        assert_eq!(result.unwrap(), 42);

        // Test failed operation
        let result: Result<i32, String> = with_metrics(
            "failed_operation",
            Some(HashMap::from([(
                "test_tag".to_string(),
                "test_value".to_string(),
            )])),
            || async { Err("Something went wrong".to_string()) },
        )
        .await;

        assert!(result.is_err());
    }
}
