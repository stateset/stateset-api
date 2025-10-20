use axum::{
    extract::Request,
    http::{HeaderName, HeaderValue},
    middleware::Next,
    response::Response,
};

// Adds a minimal, safe set of security headers to all responses.
// CSP kept relaxed since this is an API; adjust if serving docs or static content.
pub async fn security_headers_middleware(mut req: Request, next: Next) -> Response {
    let mut res = next.run(req).await;

    let headers = res.headers_mut();

    // Prevent MIME sniffing
    headers.insert(
        HeaderName::from_static("x-content-type-options"),
        HeaderValue::from_static("nosniff"),
    );

    // Basic clickjacking protection
    headers.insert(
        HeaderName::from_static("x-frame-options"),
        HeaderValue::from_static("DENY"),
    );

    // Referrer policy minimal leakage
    headers.insert(
        HeaderName::from_static("referrer-policy"),
        HeaderValue::from_static("no-referrer"),
    );

    // HSTS (only meaningful over TLS/production). Max-age ~ 6 months.
    headers.insert(
        HeaderName::from_static("strict-transport-security"),
        HeaderValue::from_static("max-age=15552000; includeSubDomains"),
    );

    res
}
