use thiserror::Error;
use actix_web::{HttpResponse, ResponseError};
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

impl ResponseError for ApiError {
    fn error_response(&self) -> HttpResponse {
        // Log the error before responding
        error!("API Error occurred: {:?}", self);

        match self {
            ApiError::InternalServerError => {
                HttpResponse::InternalServerError().json(json!({"error": "Internal Server Error"}))
            }
            ApiError::BadRequest(ref message) => {
                HttpResponse::BadRequest().json(json!({"error": "Bad Request", "details": message}))
            }
            ApiError::Unauthorized => {
                HttpResponse::Unauthorized().json(json!({"error": "Unauthorized"}))
            }
            ApiError::Forbidden => {
                HttpResponse::Forbidden().json(json!({"error": "Forbidden"}))
            }
            ApiError::NotFound => {
                HttpResponse::NotFound().json(json!({"error": "Not Found"}))
            }
            ApiError::UnprocessableEntity(ref message) => {
                HttpResponse::UnprocessableEntity().json(json!({"error": "Unprocessable Entity", "details": message}))
            }
            ApiError::TooManyRequests => {
                HttpResponse::TooManyRequests().json(json!({"error": "Too Many Requests"}))
            }
            ApiError::ServiceUnavailable(ref message) => {
                HttpResponse::ServiceUnavailable().json(json!({"error": "Service Unavailable", "details": message}))
            }
        }
    }

    fn status_code(&self) -> actix_web::http::StatusCode {
        match self {
            ApiError::InternalServerError => actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
            ApiError::BadRequest(_) => actix_web::http::StatusCode::BAD_REQUEST,
            ApiError::Unauthorized => actix_web::http::StatusCode::UNAUTHORIZED,
            ApiError::Forbidden => actix_web::http::StatusCode::FORBIDDEN,
            ApiError::NotFound => actix_web::http::StatusCode::NOT_FOUND,
            ApiError::UnprocessableEntity(_) => actix_web::http::StatusCode::UNPROCESSABLE_ENTITY,
            ApiError::TooManyRequests => actix_web::http::StatusCode::TOO_MANY_REQUESTS,
            ApiError::ServiceUnavailable(_) => actix_web::http::StatusCode::SERVICE_UNAVAILABLE,
        }
    }
}
