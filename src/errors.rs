use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json;
use sea_orm::error::DbErr;
use serde::Serialize;
use thiserror::Error;
use utoipa::ToSchema;
use uuid::Uuid;

// gRPC error mapping module
pub mod grpc;

/// Simplified error structure for OpenAPI documentation
#[derive(Debug, Serialize, ToSchema)]
pub struct ErrorResponse {
    pub error: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    pub timestamp: String,
}

#[derive(Debug, thiserror::Error, Serialize)]
pub enum ServiceError {
    #[error("Database error: {0}")]
    DatabaseError(#[from] #[serde(skip)] sea_orm::error::DbErr),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Validation error: {0}")]
    ValidationError(String),

    #[error("Authentication error: {0}")]
    AuthError(String),

    #[error("Invalid operation: {0}")]
    InvalidOperation(String),

    #[error("Event error: {0}")]
    EventError(String),

    #[error("Internal error: {0}")]
    InternalError(String),
    
    #[error("Concurrent modification: {0}")]
    ConcurrentModification(Uuid),

    #[error("Not found error: {0}")]
    NotFoundError(String),

    #[error("Order error: {0}")]
    OrderError(String),

    #[error("Inventory error: {0}")]
    InventoryError(String),

    #[error("Invalid status: {0}")]
    InvalidStatus(String),

    #[error("Internal server error")]
    InternalServerError,

    #[error("External service error: {0}")]
    ExternalServiceError(String),

    #[error("External API error: {0}")]
    ExternalApiError(String),

    #[error("Unauthorized: {0}")]
    Unauthorized(String),

    #[error("Forbidden: {0}")]
    Forbidden(String),

    #[error("JWT error: {0}")]
    JwtError(String),

    #[error("Hash error: {0}")]
    HashError(String),

    #[error("Rate limit exceeded")]
    RateLimitExceeded,
    
    #[error("Bad request: {0}")]
    BadRequest(String),
    
    #[error("Conflict: {0}")]
    Conflict(String),
    
    #[error("Insufficient stock: {0}")]
    InsufficientStock(String),
    
    #[error("Payment failed: {0}")]
    PaymentFailed(String),
    
    #[error("Cache error: {0}")]
    CacheError(String),
    
    #[error("Queue error: {0}")]
    QueueError(String),
    
    #[error("Serialization error: {0}")]
    SerializationError(String),
    
    #[error("Circuit breaker open")]
    CircuitBreakerOpen,
    
    #[error("Migration error: {0}")]
    MigrationError(String),

    #[error("Other error: {0}")]
    Other(#[from] #[serde(skip)] anyhow::Error),
}

impl From<validator::ValidationErrors> for ServiceError {
    fn from(err: validator::ValidationErrors) -> Self {
        ServiceError::ValidationError(err.to_string())
    }
}

impl From<()> for ServiceError {
    fn from(_: ()) -> Self {
        ServiceError::InternalServerError
    }
}

impl IntoResponse for ServiceError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            ServiceError::DatabaseError(ref e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
            ServiceError::NotFound(ref e) => (StatusCode::NOT_FOUND, e.to_string()),
            ServiceError::ValidationError(ref e) => (StatusCode::BAD_REQUEST, e.to_string()),
            ServiceError::AuthError(ref e) => (StatusCode::UNAUTHORIZED, e.to_string()),
            ServiceError::InvalidOperation(ref e) => (StatusCode::BAD_REQUEST, e.to_string()),
            ServiceError::EventError(ref e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
            ServiceError::InternalError(ref e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
            ServiceError::NotFoundError(ref e) => (StatusCode::NOT_FOUND, e.to_string()),
            ServiceError::OrderError(ref e) => (StatusCode::BAD_REQUEST, e.to_string()),
            ServiceError::InventoryError(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            ServiceError::InvalidStatus(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            ServiceError::InternalServerError => (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error".to_string()),
            ServiceError::ExternalServiceError(msg) => (StatusCode::BAD_GATEWAY, msg.clone()),
            ServiceError::ExternalApiError(msg) => (StatusCode::BAD_GATEWAY, msg.clone()),
            ServiceError::Unauthorized(msg) => (StatusCode::UNAUTHORIZED, msg.clone()),
            ServiceError::Forbidden(msg) => (StatusCode::FORBIDDEN, msg.clone()),
            ServiceError::JwtError(msg) => (StatusCode::UNAUTHORIZED, msg.clone()),
            ServiceError::HashError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg.clone()),
            ServiceError::RateLimitExceeded => (StatusCode::TOO_MANY_REQUESTS, "Rate limit exceeded".to_string()),
            ServiceError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            ServiceError::Conflict(msg) => (StatusCode::CONFLICT, msg.clone()),
            ServiceError::InsufficientStock(msg) => (StatusCode::UNPROCESSABLE_ENTITY, msg.clone()),
            ServiceError::PaymentFailed(msg) => (StatusCode::PAYMENT_REQUIRED, msg.clone()),
            ServiceError::CacheError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg.clone()),
            ServiceError::QueueError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg.clone()),
            ServiceError::SerializationError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg.clone()),
            ServiceError::CircuitBreakerOpen => (StatusCode::SERVICE_UNAVAILABLE, "Service temporarily unavailable".to_string()),
            ServiceError::MigrationError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg.clone()),
            ServiceError::ConcurrentModification(id) => (StatusCode::CONFLICT, format!("Concurrent modification for ID {}", id)),
            ServiceError::Other(ref e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
        };

        let body = Json(serde_json::json!({
            "error": error_message,
        }));

        (status, body).into_response()
    }
}

/// API Error type for HTTP responses
#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("Service error: {0}")]
    ServiceError(#[from] ServiceError),

    #[error("Validation error: {0}")]
    ValidationError(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Unauthorized")]
    Unauthorized,

    #[error("Internal server error")]
    InternalServerError,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, error_message) = match &self {
            ApiError::ServiceError(service_error) => {
                match service_error {
                    ServiceError::NotFound(e) | ServiceError::NotFoundError(e) => (StatusCode::NOT_FOUND, e.clone()),
                    ServiceError::ValidationError(e) | ServiceError::InvalidStatus(e) => (StatusCode::BAD_REQUEST, e.clone()),
                    ServiceError::AuthError(e) | ServiceError::JwtError(e) | ServiceError::Unauthorized(e) => (StatusCode::UNAUTHORIZED, e.clone()),
                    ServiceError::InvalidOperation(e) | ServiceError::BadRequest(e) => (StatusCode::BAD_REQUEST, e.clone()),
                    ServiceError::EventError(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.clone()),
                    ServiceError::InternalError(e) | ServiceError::HashError(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.clone()),
                    ServiceError::OrderError(e) | ServiceError::InventoryError(e) => (StatusCode::BAD_REQUEST, e.clone()),
                    ServiceError::DatabaseError(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Database error".to_string()),
                    ServiceError::InternalServerError => (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error".to_string()),
                    ServiceError::ExternalServiceError(e) | ServiceError::ExternalApiError(e) => (StatusCode::BAD_GATEWAY, e.clone()),
                    ServiceError::Forbidden(e) => (StatusCode::FORBIDDEN, e.clone()),
                    ServiceError::RateLimitExceeded => (StatusCode::TOO_MANY_REQUESTS, "Rate limit exceeded".to_string()),
                    ServiceError::Conflict(e) => (StatusCode::CONFLICT, e.clone()),
                    ServiceError::InsufficientStock(e) => (StatusCode::UNPROCESSABLE_ENTITY, e.clone()),
                    ServiceError::PaymentFailed(e) => (StatusCode::PAYMENT_REQUIRED, e.clone()),
                    ServiceError::CacheError(e) | ServiceError::QueueError(e) | ServiceError::SerializationError(e) | ServiceError::MigrationError(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.clone()),
                    ServiceError::CircuitBreakerOpen => (StatusCode::SERVICE_UNAVAILABLE, "Service temporarily unavailable".to_string()),
                    ServiceError::ConcurrentModification(id) => (StatusCode::CONFLICT, format!("Concurrent modification for ID {}", id)),
                    ServiceError::Other(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
                }
            },
            ApiError::ValidationError(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            ApiError::NotFound(msg) => (StatusCode::NOT_FOUND, msg.clone()),
            ApiError::Unauthorized => (StatusCode::UNAUTHORIZED, "Unauthorized".to_string()),
            ApiError::InternalServerError => (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error".to_string()),
        };

        let error_response = ErrorResponse {
            error: status.canonical_reason().unwrap_or("Unknown Error").to_string(),
            message: error_message,
            details: None,
            request_id: None, // Could be extracted from request extensions in a real implementation
            timestamp: chrono::Utc::now().to_rfc3339(),
        };

        (status, Json(error_response)).into_response()
    }
}

// Type aliases for backwards compatibility
pub type AppError = ServiceError;
pub type ASNError = ServiceError;
pub type InventoryError = ServiceError;
pub type OrderError = ServiceError;
pub type ReturnError = ServiceError;
pub type ShipmentError = ServiceError;
pub type WarrantyError = ServiceError;
pub type WorkOrderError = ServiceError;

// Result extensions for easier error handling
pub trait ResultExt<T> {
    fn map_err_to_service(self) -> Result<T, ServiceError>;
}

impl<T, E> ResultExt<T> for Result<T, E>
where
    E: Into<ServiceError>,
{
    fn map_err_to_service(self) -> Result<T, ServiceError> {
        self.map_err(|e| e.into())
    }
} 