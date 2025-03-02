use thiserror::Error;
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Serialize, Serializer};
use serde_json::json;
use tracing::{error, warn};
use std::fmt;

/// Represents standard API error types with detailed context
#[derive(Error, Debug)]
pub enum ApiError {
    #[error("Internal Server Error: {0}")]
    InternalServerError(String),

    #[error("Bad Request: {0}")]
    BadRequest(String),

    #[error("Unauthorized: {0}")]
    Unauthorized(String),

    #[error("Forbidden: {0}")]
    Forbidden(String),

    #[error("Not Found: {0}")]
    NotFound(String),

    #[error("Unprocessable Entity: {0}")]
    UnprocessableEntity(String),

    #[error("Too Many Requests: {0}")]
    TooManyRequests(String),

    #[error("Service Unavailable: {0}")]
    ServiceUnavailable(String),
}

/// Domain-specific error types for orders
#[derive(Error, Debug)]
pub enum OrderError {
    #[error("Order not found: {0}")]
    NotFound(String),
    
    #[error("Order validation failed: {0}")]
    ValidationFailed(String),
    
    #[error("Order processing failed: {0}")]
    ProcessingFailed(String),
}

/// Structured error response for JSON serialization
#[derive(Serialize, Debug)]
struct ErrorResponse {
    error: String,
    #[serde(serialize_with = "serialize_details")]
    details: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    code: Option<String>,
}

// Custom serializer to handle multi-line error details
fn serialize_details<S>(details: &str, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    if details.contains('\n') {
        serializer.serialize_str(&details.lines().collect::<Vec<_>>().join(" "))
    } else {
        serializer.serialize_str(details)
    }
}

impl ApiError {
    /// Creates a new InternalServerError with context
    pub fn internal_with_context<E: std::error::Error>(err: E) -> Self {
        Self::InternalServerError(err.to_string())
    }

    /// Maps the error to appropriate status code and message
    fn to_status_and_message(&self) -> (StatusCode, &'static str, Option<String>) {
        match self {
            Self::InternalServerError(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error", None),
            Self::BadRequest(_) => (StatusCode::BAD_REQUEST, "Bad Request", None),
            Self::Unauthorized(_) => (StatusCode::UNAUTHORIZED, "Unauthorized", None),
            Self::Forbidden(_) => (StatusCode::FORBIDDEN, "Forbidden", None),
            Self::NotFound(_) => (StatusCode::NOT_FOUND, "Not Found", None),
            Self::UnprocessableEntity(_) => (StatusCode::UNPROCESSABLE_ENTITY, "Unprocessable Entity", None),
            Self::TooManyRequests(_) => (StatusCode::TOO_MANY_REQUESTS, "Too Many Requests", Some("RATE_LIMIT_EXCEEDED".to_string())),
            Self::ServiceUnavailable(_) => (StatusCode::SERVICE_UNAVAILABLE, "Service Unavailable", None),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        // Log based on severity
        match &self {
            Self::InternalServerError(_) | Self::ServiceUnavailable(_) => error!(error = %self, "Critical API error"),
            Self::BadRequest(_) | Self::UnprocessableEntity(_) => warn!(error = %self, "Client error"),
            _ => tracing::info!(error = %self, "API error"),
        }

        let (status, error_message, code) = self.to_status_and_message();
        
        let body = Json(ErrorResponse {
            error: error_message.to_string(),
            details: self.to_string(),
            code,
        });

        (status, body).into_response()
    }
}

impl From<OrderError> for ApiError {
    fn from(err: OrderError) -> Self {
        match err {
            OrderError::NotFound(msg) => Self::NotFound(msg),
            OrderError::ValidationFailed(msg) => Self::UnprocessableEntity(msg),
            OrderError::ProcessingFailed(msg) => Self::InternalServerError(msg),
        }
    }
}

/// Converts any error into an ApiError with context
pub fn handle_error<E>(err: E) -> ApiError
where
    E: std::error::Error + 'static,
{
    let error_string = err.to_string();
    error!(error = %err, "Unhandled error occurred");
    
    // Attempt to downcast to specific error types
    if let Some(order_err) = err.downcast_ref::<OrderError>() {
        order_err.clone().into()
    } else {
        ApiError::InternalServerError(error_string)
    }
}

/// Extension trait for Result to simplify error handling
pub trait ResultExt<T> {
    fn map_api_err(self) -> Result<T, ApiError>;
}

impl<T, E> ResultExt<T> for Result<T, E>
where
    E: std::error::Error + 'static,
{
    fn map_api_err(self) -> Result<T, ApiError> {
        self.map_err(handle_error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;

    #[test]
    fn test_api_error_response() {
        let error = ApiError::BadRequest("Invalid input".to_string());
        let response = error.into_response();
        
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        
        let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        let json: ErrorResponse = serde_json::from_slice(&body).unwrap();
        
        assert_eq!(json.error, "Bad Request");
        assert_eq!(json.details, "Bad Request: Invalid input");
        assert_eq!(json.code, None);
    }

    #[test]
    fn test_order_error_conversion() {
        let order_error = OrderError::NotFound("Order #123".to_string());
        let api_error: ApiError = order_error.into();
        
        assert!(matches!(api_error, ApiError::NotFound(_)));
    }
}