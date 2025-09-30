use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    #[serde(rename = "type")]
    pub error_type: String,
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub param: Option<String>,
}

#[derive(Debug, Error)]
pub enum ServiceError {
    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("Invalid operation: {0}")]
    InvalidOperation(String),

    #[error("Cache error: {0}")]
    CacheError(String),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("Parse error: {0}")]
    ParseError(String),

    #[error("Internal error: {0}")]
    InternalError(String),
}

#[derive(Debug, Error)]
pub enum ApiError {
    #[error("Service error: {0}")]
    ServiceError(#[from] ServiceError),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Bad request: {message}")]
    BadRequest {
        message: String,
        error_code: Option<String>,
    },

    #[error("Method not allowed: {message}")]
    MethodNotAllowed {
        message: String,
    },

    #[error("Internal server error: {message}")]
    InternalServerError {
        message: String,
    },
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, error_type, code, message) = match &self {
            ApiError::ServiceError(service_error) => match service_error {
                ServiceError::NotFound(msg) => (
                    StatusCode::NOT_FOUND,
                    "invalid_request".to_string(),
                    "not_found".to_string(),
                    msg.clone(),
                ),
                ServiceError::InvalidInput(msg) => (
                    StatusCode::BAD_REQUEST,
                    "invalid_request".to_string(),
                    "invalid".to_string(),
                    msg.clone(),
                ),
                ServiceError::InvalidOperation(msg) => (
                    StatusCode::BAD_REQUEST,
                    "invalid_request".to_string(),
                    "invalid".to_string(),
                    msg.clone(),
                ),
                _ => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "processing_error".to_string(),
                    "internal_error".to_string(),
                    "Internal server error".to_string(),
                ),
            },
            ApiError::NotFound(msg) => (
                StatusCode::NOT_FOUND,
                "invalid_request".to_string(),
                "not_found".to_string(),
                msg.clone(),
            ),
            ApiError::BadRequest { message, error_code } => (
                StatusCode::BAD_REQUEST,
                "invalid_request".to_string(),
                error_code.clone().unwrap_or_else(|| "invalid".to_string()),
                message.clone(),
            ),
            ApiError::MethodNotAllowed { message } => (
                StatusCode::METHOD_NOT_ALLOWED,
                "invalid_request".to_string(),
                "not_allowed".to_string(),
                message.clone(),
            ),
            ApiError::InternalServerError { message } => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "processing_error".to_string(),
                "internal_error".to_string(),
                message.clone(),
            ),
        };

        let error_response = ErrorResponse {
            error_type,
            code,
            message,
            param: None,
        };

        (status, Json(error_response)).into_response()
    }
} 