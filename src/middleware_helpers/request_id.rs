use crate::tracing::RequestId;
use axum::{
    extract::Request,
    http::{header::HeaderName, HeaderValue},
    middleware::Next,
    response::Response,
};

/// Header name for the request ID
pub const REQUEST_ID_HEADER: &str = "x-request-id";

/// Middleware to add request ID to every request
pub async fn request_id_middleware(mut request: Request, next: Next) -> Response {
    // Check if request already has an ID
    let request_id = request
        .headers()
        .get(REQUEST_ID_HEADER)
        .and_then(|v| v.to_str().ok())
        .map(RequestId::new)
        .unwrap_or_else(RequestId::default);

    // Add request ID to headers (request IDs are validated ASCII, so this won't fail)
    request.headers_mut().insert(
        HeaderName::from_static(REQUEST_ID_HEADER),
        HeaderValue::from_str(request_id.as_str())
            .expect("request ID contains only valid header characters"),
    );

    // Make request id available to handlers
    request.extensions_mut().insert(request_id.clone());

    // Set tracing span with request ID
    let span = tracing::info_span!(
        "request",
        request_id = %request_id.as_str(),
        method = %request.method(),
        uri = %request.uri(),
    );
    // Process request with span
    let _guard = span.enter();
    let mut response =
        crate::tracing::scope_request_id(
            request_id.clone(),
            async move { next.run(request).await },
        )
        .await;

    // Add request ID to response headers
    response.headers_mut().insert(
        HeaderName::from_static(REQUEST_ID_HEADER),
        HeaderValue::from_str(request_id.as_str())
            .expect("request ID contains only valid header characters"),
    );

    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::{to_bytes, Body},
        extract::Extension,
        http::{Request as HttpRequest, StatusCode},
        routing::get,
        Router,
    };
    use tower::ServiceExt;

    async fn extension_handler(
        Extension(request_id): Extension<RequestId>,
    ) -> (StatusCode, String) {
        (
            StatusCode::OK,
            format!("request-id:{}", request_id.as_str()),
        )
    }

    #[tokio::test]
    async fn middleware_adds_request_id_header_and_extension() {
        let app = Router::new()
            .route("/", get(extension_handler))
            .layer(axum::middleware::from_fn(request_id_middleware));

        let response = app
            .oneshot(
                HttpRequest::builder()
                    .uri("/")
                    .method("GET")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        let header = response.headers().get(REQUEST_ID_HEADER).cloned();
        assert!(header.is_some());

        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let body_str = String::from_utf8(body.to_vec()).unwrap();
        assert!(body_str.starts_with("request-id:"));
    }
}
