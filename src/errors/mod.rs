use thiserror::Error;
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use tracing::error;

#[derive(Error, Debug)]
pub enum ApiError {
    #[error("Internal Server Error")]
    InternalServerError,

    #[error("Bad Request: {0}")]
    BadRequest(String),

    #[error("Unauthorized")]
    Unauthorized,

    #[error("Forbidden")]
    Forbidden,

    #[error("Not Found")]
    NotFound,

    #[error("Unprocessable Entity: {0}")]
    UnprocessableEntity(String),

    #[error("Too Many Requests")]
    TooManyRequests,

    #[error("Service Unavailable: {0}")]
    ServiceUnavailable(String),
}

#[derive(Error, Debug)]
pub enum OrderError {
    #[error("Order not found")]
    NotFound,
    
    // Add other specific errors related to orders here
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        // Log the error before responding
        error!("API Error occurred: {:?}", self);

        let (status, error_message) = match self {
            ApiError::InternalServerError => (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error"),
            ApiError::BadRequest(ref message) => (StatusCode::BAD_REQUEST, "Bad Request"),
            ApiError::Unauthorized => (StatusCode::UNAUTHORIZED, "Unauthorized"),
            ApiError::Forbidden => (StatusCode::FORBIDDEN, "Forbidden"),
            ApiError::NotFound => (StatusCode::NOT_FOUND, "Not Found"),
            ApiError::UnprocessableEntity(ref message) => (StatusCode::UNPROCESSABLE_ENTITY, "Unprocessable Entity"),
            ApiError::TooManyRequests => (StatusCode::TOO_MANY_REQUESTS, "Too Many Requests"),
            ApiError::ServiceUnavailable(ref message) => (StatusCode::SERVICE_UNAVAILABLE, "Service Unavailable"),
            OrderError::NotFound => (StatusCode::NOT_FOUND, "Order not found"),
        };

        let body = Json(json!({
            "error": error_message,
            "details": self.to_string(),
        }));

        (status, body).into_response()
    }
}

// Helper function to convert any error into an ApiError
pub fn handle_error<E>(err: E) -> ApiError
where
    E: std::error::Error,
{
    error!("Error occurred: {:?}", err);
    ApiError::InternalServerError
}