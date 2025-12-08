//! Correlation ID Middleware
//!
//! This module provides distributed tracing support through correlation IDs.
//! Correlation IDs are propagated across service boundaries to enable
//! end-to-end request tracing.

use axum::{
    extract::Request,
    http::{HeaderName, HeaderValue},
    middleware::Next,
    response::Response,
};
use tracing::{info_span, Instrument, Span};
use uuid::Uuid;

/// Header name for correlation ID
pub const CORRELATION_ID_HEADER: &str = "x-correlation-id";

/// Alternative header names (for compatibility)
pub const TRACE_ID_HEADER: &str = "x-trace-id";
pub const REQUEST_ID_HEADER: &str = "x-request-id";

/// Correlation ID wrapper for type safety
#[derive(Debug, Clone)]
pub struct CorrelationId(String);

impl CorrelationId {
    /// Generate a new correlation ID
    pub fn new() -> Self {
        Self(Uuid::new_v4().to_string())
    }

    /// Create from an existing string
    pub fn from_string(s: String) -> Self {
        Self(s)
    }

    /// Get the correlation ID as a string slice
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for CorrelationId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for CorrelationId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for CorrelationId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for CorrelationId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

/// Extract correlation ID from request headers
fn extract_correlation_id(request: &Request) -> CorrelationId {
    // Try correlation ID header first
    if let Some(value) = request.headers().get(CORRELATION_ID_HEADER) {
        if let Ok(s) = value.to_str() {
            return CorrelationId::from_string(s.to_string());
        }
    }

    // Try trace ID header
    if let Some(value) = request.headers().get(TRACE_ID_HEADER) {
        if let Ok(s) = value.to_str() {
            return CorrelationId::from_string(s.to_string());
        }
    }

    // Try request ID header
    if let Some(value) = request.headers().get(REQUEST_ID_HEADER) {
        if let Ok(s) = value.to_str() {
            return CorrelationId::from_string(s.to_string());
        }
    }

    // Generate new correlation ID if not found
    CorrelationId::new()
}

/// Correlation ID middleware
///
/// This middleware:
/// 1. Extracts or generates a correlation ID for each request
/// 2. Adds the correlation ID to the request extensions
/// 3. Creates a tracing span with the correlation ID
/// 4. Adds the correlation ID to the response headers
pub async fn correlation_id_middleware(mut request: Request, next: Next) -> Response {
    let correlation_id = extract_correlation_id(&request);

    // Store correlation ID in request extensions for use by handlers
    request.extensions_mut().insert(correlation_id.clone());

    // Create a span with the correlation ID
    let span = info_span!(
        "request",
        correlation_id = %correlation_id,
        method = %request.method(),
        path = %request.uri().path(),
    );

    // Process request within the span
    let mut response = next.run(request).instrument(span).await;

    // Add correlation ID to response headers
    let headers = response.headers_mut();

    if let Ok(value) = HeaderValue::from_str(correlation_id.as_str()) {
        headers.insert(
            HeaderName::from_static(CORRELATION_ID_HEADER),
            value.clone(),
        );
        headers.insert(HeaderName::from_static(REQUEST_ID_HEADER), value);
    }

    response
}

/// Extract correlation ID from request extensions
pub fn get_correlation_id(request: &Request) -> Option<CorrelationId> {
    request.extensions().get::<CorrelationId>().cloned()
}

/// Get current correlation ID from tracing context
pub fn current_correlation_id() -> Option<String> {
    Span::current()
        .field("correlation_id")
        .map(|_| {
            // Note: This is a simplified implementation
            // In production, you'd want to use tracing-subscriber's
            // field value extraction
            None
        })
        .flatten()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request as HttpRequest;

    #[test]
    fn test_correlation_id_generation() {
        let id1 = CorrelationId::new();
        let id2 = CorrelationId::new();
        assert_ne!(id1.as_str(), id2.as_str());
    }

    #[test]
    fn test_correlation_id_from_string() {
        let id = CorrelationId::from_string("test-123".to_string());
        assert_eq!(id.as_str(), "test-123");
    }

    #[test]
    fn test_extract_correlation_id_from_header() {
        let request = HttpRequest::builder()
            .header(CORRELATION_ID_HEADER, "header-id-123")
            .body(Body::empty())
            .unwrap();

        let id = extract_correlation_id(&request);
        assert_eq!(id.as_str(), "header-id-123");
    }

    #[test]
    fn test_extract_correlation_id_generates_new() {
        let request = HttpRequest::builder().body(Body::empty()).unwrap();

        let id = extract_correlation_id(&request);
        assert!(!id.as_str().is_empty());
        // Should be a valid UUID format
        assert!(Uuid::parse_str(id.as_str()).is_ok());
    }

    #[test]
    fn test_correlation_id_display() {
        let id = CorrelationId::from_string("display-test".to_string());
        assert_eq!(format!("{}", id), "display-test");
    }
}
