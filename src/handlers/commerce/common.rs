use crate::errors::{ApiError, ServiceError};
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;
use validator::Validate;

/// Standard success response
pub fn success_response<T: Serialize>(data: T) -> Response {
    (StatusCode::OK, Json(data)).into_response()
}

/// Standard created response
pub fn created_response<T: Serialize>(data: T) -> Response {
    (StatusCode::CREATED, Json(data)).into_response()
}

/// Validate request input
pub fn validate_input<T: Validate>(input: &T) -> Result<(), ApiError> {
    input.validate().map_err(|e| ApiError::ValidationError {
        message: format!("Validation failed: {}", e),
        error_code: None,
    })
}

/// Map service errors to API errors
pub fn map_service_error(err: ServiceError) -> ApiError {
    match err {
        ServiceError::NotFound(msg) => ApiError::NotFound {
            message: msg,
            error_code: None,
        },
        ServiceError::ValidationError(msg) => ApiError::ValidationError {
            message: msg,
            error_code: None,
        },
        ServiceError::AuthError(msg) => ApiError::AuthError {
            message: msg,
            error_code: None,
        },
        ServiceError::InvalidOperation(msg) => ApiError::BadRequest {
            message: msg,
            error_code: None,
        },
        _ => ApiError::InternalServerError {
            message: "An unexpected error occurred".to_string(),
            error_code: None,
        },
    }
} 