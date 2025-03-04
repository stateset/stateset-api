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
use uuid::Uuid;

/// Application-level error type
#[derive(Error, Debug)]
pub enum AppError {
    #[error("Database error: {0}")]
    DatabaseError(String),
    
    #[error("Migration error: {0}")]
    MigrationError(String),
    
    #[error("Internal error: {0}")]
    InternalError(String),
}

impl From<AppError> for ApiError {
    fn from(err: AppError) -> Self {
        match err {
            AppError::DatabaseError(msg) => Self::InternalServerError(msg),
            AppError::MigrationError(msg) => Self::InternalServerError(msg),
            AppError::InternalError(msg) => Self::InternalServerError(msg),
        }
    }
}

impl From<anyhow::Error> for AppError {
    fn from(err: anyhow::Error) -> Self {
        Self::InternalError(format!("Context: {}", err))
    }
}

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

    #[error("Conflict: {0}")]
    Conflict(String),

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

/// Domain-specific error types for inventory operations
#[derive(Error, Debug, Clone)]
pub enum InventoryError {
    #[error("Inventory not found: {0}")]
    NotFound(String),
    
    #[error("Duplicate reservation for reference {0}")]
    DuplicateReservation(Uuid),
    
    #[error("Duplicate allocation for reference {0}")]
    DuplicateAllocation(Uuid),
    
    #[error("Insufficient inventory for product {0}")]
    InsufficientInventory(Uuid),
    
    #[error("Would result in negative inventory for product {0}")]
    NegativeInventory(Uuid),
    
    #[error("Invalid reason code: {0}")]
    InvalidReasonCode(String),
    
    #[error("Concurrent modification of inventory {0}")]
    ConcurrentModification(Uuid),
    
    #[error("Database error: {0}")]
    DatabaseError(String),
    
    #[error("Event error: {0}")]
    EventError(String),
    
    #[error("Validation error: {0}")]
    ValidationError(String),
}

/// Implementation for InventoryError
impl InventoryError {
    /// Helper method to get a string representation of the error type
    /// Useful for metrics and logging
    pub fn error_type(&self) -> &str {
        match self {
            Self::NotFound(_) => "not_found",
            Self::DuplicateReservation(_) => "duplicate_reservation",
            Self::DuplicateAllocation(_) => "duplicate_allocation",
            Self::InsufficientInventory(_) => "insufficient_inventory",
            Self::NegativeInventory(_) => "negative_inventory",
            Self::InvalidReasonCode(_) => "invalid_reason_code",
            Self::ConcurrentModification(_) => "concurrent_modification",
            Self::DatabaseError(_) => "database_error",
            Self::EventError(_) => "event_error",
            Self::ValidationError(_) => "validation_error",
        }
    }
}

/// Service layer errors for internal use
#[derive(Error, Debug)]
pub enum ServiceError {
    #[error("Database error: {0}")]
    DatabaseError(String),
    
    #[error("Validation error: {0}")]
    ValidationError(String),
    
    #[error("Not found: {0}")]
    NotFoundError(String),
    
    #[error("Authentication error: {0}")]
    AuthError(String),
    
    #[error("Authorization error: {0}")]
    ForbiddenError(String),
    
    #[error("Event publishing error: {0}")]
    EventError(String),
    
    #[error("External service error: {0}")]
    ExternalServiceError(String),
    
    #[error("Cache error: {0}")]
    CacheError(String),
    
    #[error("Rate limit exceeded: {0}")]
    RateLimitError(String),
    
    #[error("Circuit breaker open: {0}")]
    CircuitBreakerError(String),
    
    #[error("Migration error: {0}")]
    MigrationError(String),
    
    #[error("Internal error: {0}")]
    InternalError(String),
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
            Self::Conflict(_) => (StatusCode::CONFLICT, "Conflict", Some("CONCURRENT_MODIFICATION".to_string())),
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

impl From<ServiceError> for ApiError {
    fn from(err: ServiceError) -> Self {
        match err {
            ServiceError::DatabaseError(msg) => Self::InternalServerError(msg),
            ServiceError::ValidationError(msg) => Self::UnprocessableEntity(msg),
            ServiceError::NotFoundError(msg) => Self::NotFound(msg),
            ServiceError::AuthError(msg) => Self::Unauthorized(msg),
            ServiceError::ForbiddenError(msg) => Self::Forbidden(msg),
            ServiceError::EventError(msg) => Self::InternalServerError(msg),
            ServiceError::ExternalServiceError(msg) => Self::ServiceUnavailable(msg),
            ServiceError::CacheError(msg) => Self::InternalServerError(msg),
            ServiceError::RateLimitError(msg) => Self::TooManyRequests(msg),
            ServiceError::CircuitBreakerError(msg) => Self::ServiceUnavailable(msg),
            ServiceError::MigrationError(msg) => Self::InternalServerError(msg),
            ServiceError::InternalError(msg) => Self::InternalServerError(msg),
        }
    }
}

impl From<InventoryError> for ApiError {
    fn from(err: InventoryError) -> Self {
        match err {
            InventoryError::NotFound(msg) => Self::NotFound(msg),
            InventoryError::DuplicateReservation(id) => Self::BadRequest(format!("Duplicate reservation for reference {}", id)),
            InventoryError::DuplicateAllocation(id) => Self::BadRequest(format!("Duplicate allocation for reference {}", id)),
            InventoryError::InsufficientInventory(product_id) => Self::UnprocessableEntity(format!("Insufficient inventory for product {}", product_id)),
            InventoryError::NegativeInventory(product_id) => Self::UnprocessableEntity(format!("Would result in negative inventory for product {}", product_id)),
            InventoryError::InvalidReasonCode(code) => Self::BadRequest(format!("Invalid reason code: {}", code)),
            InventoryError::ConcurrentModification(id) => Self::Conflict(format!("Concurrent modification of inventory {}", id)),
            InventoryError::DatabaseError(msg) => Self::InternalServerError(msg),
            InventoryError::EventError(msg) => Self::InternalServerError(msg),
            InventoryError::ValidationError(msg) => Self::BadRequest(msg),
        }
    }
}

impl From<InventoryError> for ServiceError {
    fn from(err: InventoryError) -> Self {
        match err {
            InventoryError::NotFound(msg) => Self::NotFoundError(msg),
            InventoryError::DuplicateReservation(id) => Self::ValidationError(format!("Duplicate reservation for reference {}", id)),
            InventoryError::DuplicateAllocation(id) => Self::ValidationError(format!("Duplicate allocation for reference {}", id)),
            InventoryError::InsufficientInventory(product_id) => Self::ValidationError(format!("Insufficient inventory for product {}", product_id)),
            InventoryError::NegativeInventory(product_id) => Self::ValidationError(format!("Would result in negative inventory for product {}", product_id)),
            InventoryError::InvalidReasonCode(code) => Self::ValidationError(format!("Invalid reason code: {}", code)),
            InventoryError::ConcurrentModification(id) => Self::ValidationError(format!("Concurrent modification of inventory {}", id)),
            InventoryError::DatabaseError(msg) => Self::DatabaseError(msg),
            InventoryError::EventError(msg) => Self::EventError(msg),
            InventoryError::ValidationError(msg) => Self::ValidationError(msg),
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
    } else if let Some(inventory_err) = err.downcast_ref::<InventoryError>() {
        inventory_err.clone().into()
    } else if let Some(service_err) = err.downcast_ref::<ServiceError>() {
        service_err.clone().into()
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

    // Simple test for error conversion
    #[test]
    fn test_order_error_conversion() {
        let order_error = OrderError::NotFound("Order #123".to_string());
        let api_error: ApiError = order_error.into();
        
        assert!(matches!(api_error, ApiError::NotFound(_)));
    }
    
    // Test AppError conversion
    #[test]
    fn test_app_error_conversion() {
        let app_error = AppError::DatabaseError("Connection failed".to_string());
        let api_error: ApiError = app_error.into();
        
        assert!(matches!(api_error, ApiError::InternalServerError(_)));
    }
}