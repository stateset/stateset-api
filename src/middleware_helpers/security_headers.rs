use axum::{
    extract::Request,
    http::{HeaderName, HeaderValue},
    middleware::Next,
    response::Response,
};

/// Comprehensive security headers middleware for production-grade API security.
///
/// This middleware adds all recommended security headers following OWASP guidelines
/// and industry best practices for API security.
pub async fn security_headers_middleware(req: Request, next: Next) -> Response {
    let mut res = next.run(req).await;

    let headers = res.headers_mut();

    // Prevent MIME sniffing - stops browsers from trying to guess content types
    headers.insert(
        HeaderName::from_static("x-content-type-options"),
        HeaderValue::from_static("nosniff"),
    );

    // Clickjacking protection - prevents embedding in frames
    headers.insert(
        HeaderName::from_static("x-frame-options"),
        HeaderValue::from_static("DENY"),
    );

    // Referrer policy - minimal information leakage
    headers.insert(
        HeaderName::from_static("referrer-policy"),
        HeaderValue::from_static("strict-origin-when-cross-origin"),
    );

    // HSTS - enforce HTTPS for 1 year (31536000 seconds)
    // Includes subdomains and allows preload list inclusion
    headers.insert(
        HeaderName::from_static("strict-transport-security"),
        HeaderValue::from_static("max-age=31536000; includeSubDomains; preload"),
    );

    // Content Security Policy - restrictive policy for API
    // Allows only self-origin and blocks all frame ancestors
    headers.insert(
        HeaderName::from_static("content-security-policy"),
        HeaderValue::from_static(
            "default-src 'none'; frame-ancestors 'none'; form-action 'none'; base-uri 'none'",
        ),
    );

    // XSS Protection - legacy but still useful for older browsers
    headers.insert(
        HeaderName::from_static("x-xss-protection"),
        HeaderValue::from_static("1; mode=block"),
    );

    // Prevent cross-domain policies
    headers.insert(
        HeaderName::from_static("x-permitted-cross-domain-policies"),
        HeaderValue::from_static("none"),
    );

    // Remove server identification (set generic value)
    headers.insert(
        HeaderName::from_static("server"),
        HeaderValue::from_static("StateSet-API"),
    );

    // Permissions Policy - disable unnecessary browser features
    headers.insert(
        HeaderName::from_static("permissions-policy"),
        HeaderValue::from_static(
            "accelerometer=(), camera=(), geolocation=(), gyroscope=(), magnetometer=(), microphone=(), payment=(), usb=()"
        ),
    );

    // Cache control for API responses - prevent caching of sensitive data
    if !headers.contains_key("cache-control") {
        headers.insert(
            HeaderName::from_static("cache-control"),
            HeaderValue::from_static("no-store, no-cache, must-revalidate, private"),
        );
    }

    // Pragma header for HTTP/1.0 compatibility
    if !headers.contains_key("pragma") {
        headers.insert(
            HeaderName::from_static("pragma"),
            HeaderValue::from_static("no-cache"),
        );
    }

    res
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, routing::get, Router};
    use tower::ServiceExt;

    async fn test_handler() -> &'static str {
        "ok"
    }

    #[tokio::test]
    async fn test_security_headers_are_set() {
        let app = Router::new()
            .route("/", get(test_handler))
            .layer(axum::middleware::from_fn(security_headers_middleware));

        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        let headers = response.headers();

        assert_eq!(headers.get("x-content-type-options").unwrap(), "nosniff");
        assert_eq!(headers.get("x-frame-options").unwrap(), "DENY");
        assert_eq!(
            headers.get("strict-transport-security").unwrap(),
            "max-age=31536000; includeSubDomains; preload"
        );
        assert!(headers.contains_key("content-security-policy"));
        assert!(headers.contains_key("permissions-policy"));
    }
}
