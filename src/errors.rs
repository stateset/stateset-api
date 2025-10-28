use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use sea_orm::error::DbErr;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

// gRPC error mapping module
pub mod grpc;

fn current_request_id() -> Option<String> {
    crate::tracing::current_request_id().map(|rid| rid.as_str().to_string())
}

/// Simplified error structure for OpenAPI documentation
#[derive(Debug, Serialize, Deserialize, ToSchema)]
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
    DatabaseError(
        #[from]
        #[serde(skip)]
        sea_orm::error::DbErr,
    ),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Validation error: {0}")]
    ValidationError(String),

    #[error("Authentication error: {0}")]
    AuthError(String),

    #[error("Invalid operation: {0}")]
    InvalidOperation(String),

    #[error("Invalid input: {0}")]
    InvalidInput(String),

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
    Other(
        #[from]
        #[serde(skip)]
        anyhow::Error,
    ),
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

pub trait IntoDbErr {
    fn into_db_err(self) -> DbErr;
}

impl IntoDbErr for DbErr {
    fn into_db_err(self) -> DbErr {
        self
    }
}

impl IntoDbErr for String {
    fn into_db_err(self) -> DbErr {
        DbErr::Custom(self)
    }
}

impl IntoDbErr for &str {
    fn into_db_err(self) -> DbErr {
        DbErr::Custom(self.to_string())
    }
}

impl ServiceError {
    /// Generic constructor that normalizes any supported database error input.
    pub fn db_error<E: IntoDbErr>(error: E) -> Self {
        ServiceError::DatabaseError(error.into_db_err())
    }

    /// Convenience constructor for wrapping string-based database errors.
    pub fn database_error_message(message: impl Into<String>) -> Self {
        ServiceError::db_error(message.into())
    }
}

impl IntoResponse for ServiceError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            ServiceError::DatabaseError(ref e) => {
                (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
            }
            ServiceError::NotFound(ref e) => (StatusCode::NOT_FOUND, e.to_string()),
            ServiceError::ValidationError(ref e) => (StatusCode::BAD_REQUEST, e.to_string()),
            ServiceError::AuthError(ref e) => (StatusCode::UNAUTHORIZED, e.to_string()),
            ServiceError::InvalidOperation(ref e) => (StatusCode::BAD_REQUEST, e.to_string()),
            ServiceError::InvalidInput(ref e) => (StatusCode::BAD_REQUEST, e.to_string()),
            ServiceError::EventError(ref e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
            ServiceError::InternalError(ref e) => {
                (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
            }
            ServiceError::NotFoundError(ref e) => (StatusCode::NOT_FOUND, e.to_string()),
            ServiceError::OrderError(ref e) => (StatusCode::BAD_REQUEST, e.to_string()),
            ServiceError::InventoryError(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            ServiceError::InvalidStatus(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            ServiceError::InternalServerError => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal server error".to_string(),
            ),
            ServiceError::ExternalServiceError(msg) => (StatusCode::BAD_GATEWAY, msg.clone()),
            ServiceError::ExternalApiError(msg) => (StatusCode::BAD_GATEWAY, msg.clone()),
            ServiceError::Unauthorized(msg) => (StatusCode::UNAUTHORIZED, msg.clone()),
            ServiceError::Forbidden(msg) => (StatusCode::FORBIDDEN, msg.clone()),
            ServiceError::JwtError(msg) => (StatusCode::UNAUTHORIZED, msg.clone()),
            ServiceError::HashError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg.clone()),
            ServiceError::RateLimitExceeded => (
                StatusCode::TOO_MANY_REQUESTS,
                "Rate limit exceeded".to_string(),
            ),
            ServiceError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            ServiceError::Conflict(msg) => (StatusCode::CONFLICT, msg.clone()),
            ServiceError::InsufficientStock(msg) => (StatusCode::UNPROCESSABLE_ENTITY, msg.clone()),
            ServiceError::PaymentFailed(msg) => (StatusCode::PAYMENT_REQUIRED, msg.clone()),
            ServiceError::CacheError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg.clone()),
            ServiceError::QueueError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg.clone()),
            ServiceError::SerializationError(msg) => {
                (StatusCode::INTERNAL_SERVER_ERROR, msg.clone())
            }
            ServiceError::CircuitBreakerOpen => (
                StatusCode::SERVICE_UNAVAILABLE,
                "Service temporarily unavailable".to_string(),
            ),
            ServiceError::MigrationError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg.clone()),
            ServiceError::ConcurrentModification(id) => (
                StatusCode::CONFLICT,
                format!("Concurrent modification for ID {}", id),
            ),
            ServiceError::Other(ref e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
        };

        let request_id = current_request_id();
        // Build standardized error response
        let err = ErrorResponse {
            error: status.canonical_reason().unwrap_or("Error").to_string(),
            message: error_message,
            details: None,
            request_id,
            timestamp: chrono::Utc::now().to_rfc3339(),
        };

        (status, Json(err)).into_response()
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

    #[error("Bad request: {message}")]
    BadRequest {
        message: String,
        error_code: Option<String>,
    },

    #[error("Method not allowed: {message}")]
    MethodNotAllowed { message: String },
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, error_message) = match &self {
            ApiError::ServiceError(service_error) => match service_error {
                ServiceError::NotFound(e) | ServiceError::NotFoundError(e) => {
                    (StatusCode::NOT_FOUND, e.clone())
                }
                ServiceError::ValidationError(e) | ServiceError::InvalidStatus(e) => {
                    (StatusCode::BAD_REQUEST, e.clone())
                }
                ServiceError::AuthError(e)
                | ServiceError::JwtError(e)
                | ServiceError::Unauthorized(e) => (StatusCode::UNAUTHORIZED, e.clone()),
                ServiceError::InvalidOperation(e)
                | ServiceError::BadRequest(e)
                | ServiceError::InvalidInput(e) => (StatusCode::BAD_REQUEST, e.clone()),
                ServiceError::EventError(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.clone()),
                ServiceError::InternalError(e) | ServiceError::HashError(e) => {
                    (StatusCode::INTERNAL_SERVER_ERROR, e.clone())
                }
                ServiceError::OrderError(e) | ServiceError::InventoryError(e) => {
                    (StatusCode::BAD_REQUEST, e.clone())
                }
                ServiceError::DatabaseError(_) => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Database error".to_string(),
                ),
                ServiceError::InternalServerError => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Internal server error".to_string(),
                ),
                ServiceError::ExternalServiceError(e) | ServiceError::ExternalApiError(e) => {
                    (StatusCode::BAD_GATEWAY, e.clone())
                }
                ServiceError::Forbidden(e) => (StatusCode::FORBIDDEN, e.clone()),
                ServiceError::RateLimitExceeded => (
                    StatusCode::TOO_MANY_REQUESTS,
                    "Rate limit exceeded".to_string(),
                ),
                ServiceError::Conflict(e) => (StatusCode::CONFLICT, e.clone()),
                ServiceError::InsufficientStock(e) => (StatusCode::UNPROCESSABLE_ENTITY, e.clone()),
                ServiceError::PaymentFailed(e) => (StatusCode::PAYMENT_REQUIRED, e.clone()),
                ServiceError::CacheError(e)
                | ServiceError::QueueError(e)
                | ServiceError::SerializationError(e)
                | ServiceError::MigrationError(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.clone()),
                ServiceError::CircuitBreakerOpen => (
                    StatusCode::SERVICE_UNAVAILABLE,
                    "Service temporarily unavailable".to_string(),
                ),
                ServiceError::ConcurrentModification(id) => (
                    StatusCode::CONFLICT,
                    format!("Concurrent modification for ID {}", id),
                ),
                ServiceError::Other(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
            },
            ApiError::ValidationError(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            ApiError::NotFound(msg) => (StatusCode::NOT_FOUND, msg.clone()),
            ApiError::Unauthorized => (StatusCode::UNAUTHORIZED, "Unauthorized".to_string()),
            ApiError::InternalServerError => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal server error".to_string(),
            ),
            ApiError::BadRequest { message, .. } => (StatusCode::BAD_REQUEST, message.clone()),
            ApiError::MethodNotAllowed { message } => {
                (StatusCode::METHOD_NOT_ALLOWED, message.clone())
            }
        };

        let request_id = current_request_id();
        let error_response = ErrorResponse {
            error: status
                .canonical_reason()
                .unwrap_or("Unknown Error")
                .to_string(),
            message: error_message,
            details: None,
            request_id,
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

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::to_bytes, http::StatusCode};

    #[tokio::test]
    async fn service_error_response_includes_request_id() {
        let response =
            crate::tracing::scope_request_id(crate::tracing::RequestId::new("req-123"), async {
                ServiceError::NotFound("missing".into()).into_response()
            })
            .await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);

        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let payload: ErrorResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(payload.request_id.as_deref(), Some("req-123"));
    }

    #[tokio::test]
    async fn api_error_response_includes_request_id() {
        let response =
            crate::tracing::scope_request_id(crate::tracing::RequestId::new("req-api-42"), async {
                ApiError::ServiceError(ServiceError::Forbidden("nope".into())).into_response()
            })
            .await;
        assert_eq!(response.status(), StatusCode::FORBIDDEN);

        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let payload: ErrorResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(payload.request_id.as_deref(), Some("req-api-42"));
    }
}
