use axum::{
    extract::Request,
    http::{header::HeaderName, HeaderValue},
    middleware::Next,
    response::Response,
};
use uuid::Uuid;

/// Header name for the request ID
pub const REQUEST_ID_HEADER: &str = "x-request-id";

/// Middleware to add request ID to every request
pub async fn request_id_middleware(
    mut request: Request,
    next: Next,
) -> Response {
    // Check if request already has an ID
    let request_id = request
        .headers()
        .get(REQUEST_ID_HEADER)
        .and_then(|v| v.to_str().ok())
        .map(String::from)
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    
    // Add request ID to headers
    request.headers_mut().insert(
        HeaderName::from_static(REQUEST_ID_HEADER),
        HeaderValue::from_str(&request_id).unwrap(),
    );
    
    // Set tracing span with request ID
    let span = tracing::info_span!(
        "request",
        request_id = %request_id,
        method = %request.method(),
        uri = %request.uri(),
    );
    
    // Process request with span
    let _guard = span.enter();
    let mut response = next.run(request).await;
    
    // Add request ID to response headers
    response.headers_mut().insert(
        HeaderName::from_static(REQUEST_ID_HEADER),
        HeaderValue::from_str(&request_id).unwrap(),
    );
    
    response
}