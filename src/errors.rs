use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use sea_orm::error::DbErr;
use serde::{Deserialize, Serialize};
use serde_json::json;
use utoipa::ToSchema;
use uuid::Uuid;

// gRPC error mapping module
pub mod grpc;

fn current_request_id() -> Option<String> {
    crate::tracing::current_request_id().map(|rid| rid.as_str().to_string())
}

/// Simplified error structure for OpenAPI documentation
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[schema(example = json!({
    "error": "Not Found",
    "message": "Order with ID 550e8400-e29b-41d4-a716-446655440000 not found",
    "details": null,
    "request_id": "req-abc123xyz",
    "timestamp": "2024-12-09T10:30:00.000Z"
}))]
pub struct ErrorResponse {
    /// HTTP status category (e.g., "Not Found", "Bad Request", "Internal Server Error")
    #[schema(example = "Not Found")]
    pub error: String,
    /// Human-readable error description
    #[schema(example = "Order with ID 550e8400-e29b-41d4-a716-446655440000 not found")]
    pub message: String,
    /// Additional error details (validation errors, stack traces in dev mode)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "Field 'email' must be a valid email address")]
    pub details: Option<String>,
    /// Unique request identifier for support and debugging
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "req-abc123xyz")]
    pub request_id: Option<String>,
    /// ISO 8601 timestamp when error occurred
    #[schema(example = "2024-12-09T10:30:00.000Z")]
    pub timestamp: String,
}

/// ACP-compliant error response format
/// Matches the Agentic Commerce Protocol error specification
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[schema(example = json!({
    "error": {
        "type": "invalid_request_error",
        "code": "validation_error",
        "message": "The 'quantity' field must be a positive integer",
        "param": "quantity"
    }
}))]
pub struct ACPErrorResponse {
    /// Error details object
    pub error: ACPErrorDetails,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[schema(example = json!({
    "type": "invalid_request_error",
    "code": "validation_error",
    "message": "The 'quantity' field must be a positive integer",
    "param": "quantity"
}))]
pub struct ACPErrorDetails {
    /// Error type category
    #[serde(rename = "type")]
    #[schema(example = "invalid_request_error")]
    pub error_type: ACPErrorType,
    /// Machine-readable error code
    #[schema(example = "validation_error")]
    pub code: String,
    /// Human-readable error message
    #[schema(example = "The 'quantity' field must be a positive integer")]
    pub message: String,
    /// Parameter that caused the error (for validation errors)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "quantity")]
    pub param: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ACPErrorType {
    /// Invalid request parameters or payload
    InvalidRequestError,
    /// Authentication or authorization failure
    AuthenticationError,
    /// Rate limit exceeded
    RateLimitError,
    /// General API error
    ApiError,
}

impl ACPErrorResponse {
    pub fn invalid_request(code: &str, message: String, param: Option<String>) -> Self {
        Self {
            error: ACPErrorDetails {
                error_type: ACPErrorType::InvalidRequestError,
                code: code.to_string(),
                message,
                param,
            },
        }
    }

    pub fn authentication_error(message: String) -> Self {
        Self {
            error: ACPErrorDetails {
                error_type: ACPErrorType::AuthenticationError,
                code: "authentication_failed".to_string(),
                message,
                param: None,
            },
        }
    }

    pub fn rate_limit_error() -> Self {
        Self {
            error: ACPErrorDetails {
                error_type: ACPErrorType::RateLimitError,
                code: "rate_limit_exceeded".to_string(),
                message: "Too many requests".to_string(),
                param: None,
            },
        }
    }

    pub fn api_error(code: &str, message: String) -> Self {
        Self {
            error: ACPErrorDetails {
                error_type: ACPErrorType::ApiError,
                code: code.to_string(),
                message,
                param: None,
            },
        }
    }
}

/// Convert ServiceError to ACP error response
impl From<&ServiceError> for ACPErrorResponse {
    fn from(error: &ServiceError) -> Self {
        match error {
            ServiceError::ValidationError(msg) => {
                ACPErrorResponse::invalid_request("validation_error", msg.clone(), None)
            }
            ServiceError::InvalidInput(msg) => {
                ACPErrorResponse::invalid_request("invalid_input", msg.clone(), None)
            }
            ServiceError::InvalidOperation(msg) => {
                ACPErrorResponse::invalid_request("invalid_operation", msg.clone(), None)
            }
            ServiceError::NotFound(msg) => {
                ACPErrorResponse::invalid_request("resource_not_found", msg.clone(), None)
            }
            ServiceError::AuthError(msg) | ServiceError::Unauthorized(msg) => {
                ACPErrorResponse::authentication_error(msg.clone())
            }
            ServiceError::Forbidden(msg) => {
                ACPErrorResponse::authentication_error(format!("Forbidden: {}", msg))
            }
            ServiceError::RateLimitExceeded => ACPErrorResponse::rate_limit_error(),
            ServiceError::PaymentFailed(msg) => {
                ACPErrorResponse::api_error("payment_failed", msg.clone())
            }
            ServiceError::InsufficientStock(msg) => {
                ACPErrorResponse::invalid_request("insufficient_stock", msg.clone(), None)
            }
            ServiceError::Conflict(msg) => {
                ACPErrorResponse::invalid_request("conflict", msg.clone(), None)
            }
            _ => ACPErrorResponse::api_error("internal_error", error.to_string()),
        }
    }
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

    #[error("Service unavailable: {0}")]
    ServiceUnavailable(String),

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

    /// Returns the HTTP status code for this error.
    /// This is the single source of truth for error-to-status mapping.
    pub fn status_code(&self) -> StatusCode {
        match self {
            Self::DatabaseError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::NotFound(_) => StatusCode::NOT_FOUND,
            Self::ValidationError(_)
            | Self::InvalidOperation(_)
            | Self::InvalidInput(_)
            | Self::OrderError(_)
            | Self::InventoryError(_)
            | Self::InvalidStatus(_)
            | Self::BadRequest(_) => StatusCode::BAD_REQUEST,
            Self::AuthError(_) | Self::Unauthorized(_) | Self::JwtError(_) => {
                StatusCode::UNAUTHORIZED
            }
            Self::Forbidden(_) => StatusCode::FORBIDDEN,
            Self::EventError(_)
            | Self::InternalError(_)
            | Self::InternalServerError
            | Self::HashError(_)
            | Self::CacheError(_)
            | Self::QueueError(_)
            | Self::SerializationError(_)
            | Self::MigrationError(_)
            | Self::Other(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::ExternalServiceError(_) | Self::ExternalApiError(_) => StatusCode::BAD_GATEWAY,
            Self::RateLimitExceeded => StatusCode::TOO_MANY_REQUESTS,
            Self::Conflict(_) | Self::ConcurrentModification(_) => StatusCode::CONFLICT,
            Self::InsufficientStock(_) => StatusCode::UNPROCESSABLE_ENTITY,
            Self::PaymentFailed(_) => StatusCode::PAYMENT_REQUIRED,
            Self::CircuitBreakerOpen | Self::ServiceUnavailable(_) => {
                StatusCode::SERVICE_UNAVAILABLE
            }
        }
    }

    /// Returns the error message suitable for HTTP responses.
    /// Internal errors return generic messages to avoid leaking implementation details.
    pub fn response_message(&self) -> String {
        match self {
            // For internal errors, return generic messages to avoid leaking details
            Self::DatabaseError(_) => "Database error".to_string(),
            Self::HashError(_)
            | Self::CacheError(_)
            | Self::QueueError(_)
            | Self::SerializationError(_)
            | Self::MigrationError(_)
            | Self::Other(_) => "Internal server error".to_string(),
            Self::InternalServerError => "Internal server error".to_string(),
            Self::RateLimitExceeded => "Rate limit exceeded".to_string(),
            Self::CircuitBreakerOpen => "Service temporarily unavailable".to_string(),
            Self::ServiceUnavailable(msg) => format!("Service unavailable: {}", msg),
            Self::ConcurrentModification(id) => {
                format!("Concurrent modification for ID {}", id)
            }
            // For user-facing errors, return the actual message
            _ => self.to_string(),
        }
    }
}

impl IntoResponse for ServiceError {
    fn into_response(self) -> Response {
        let status = self.status_code();
        let error_message = self.response_message();

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

    #[error("ACP error response")]
    Acp {
        status: StatusCode,
        error: ACPErrorResponse,
    },
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        // Delegate to ServiceError's unified status/message methods when applicable
        let (status, error_message) = match &self {
            ApiError::ServiceError(service_error) => (
                service_error.status_code(),
                service_error.response_message(),
            ),
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
            ApiError::Acp { status, error } => {
                return (*status, Json(error)).into_response();
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

impl ApiError {
    pub fn acp(status: StatusCode, error: ACPErrorResponse) -> Self {
        Self::Acp { status, error }
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

        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
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

        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let payload: ErrorResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(payload.request_id.as_deref(), Some("req-api-42"));
    }

    #[test]
    fn service_error_status_code_mapping() {
        // Test all major error variants map to correct status codes
        assert_eq!(
            ServiceError::NotFound("x".into()).status_code(),
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            ServiceError::ValidationError("x".into()).status_code(),
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            ServiceError::Unauthorized("x".into()).status_code(),
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            ServiceError::Forbidden("x".into()).status_code(),
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            ServiceError::RateLimitExceeded.status_code(),
            StatusCode::TOO_MANY_REQUESTS
        );
        assert_eq!(
            ServiceError::Conflict("x".into()).status_code(),
            StatusCode::CONFLICT
        );
        assert_eq!(
            ServiceError::InsufficientStock("x".into()).status_code(),
            StatusCode::UNPROCESSABLE_ENTITY
        );
        assert_eq!(
            ServiceError::PaymentFailed("x".into()).status_code(),
            StatusCode::PAYMENT_REQUIRED
        );
        assert_eq!(
            ServiceError::CircuitBreakerOpen.status_code(),
            StatusCode::SERVICE_UNAVAILABLE
        );
        assert_eq!(
            ServiceError::ExternalServiceError("x".into()).status_code(),
            StatusCode::BAD_GATEWAY
        );
        assert_eq!(
            ServiceError::InternalServerError.status_code(),
            StatusCode::INTERNAL_SERVER_ERROR
        );
    }

    #[test]
    fn service_error_response_message_hides_internal_details() {
        // Internal errors should NOT expose implementation details
        assert_eq!(
            ServiceError::HashError("sensitive".into()).response_message(),
            "Internal server error"
        );
        assert_eq!(
            ServiceError::CacheError("redis failed".into()).response_message(),
            "Internal server error"
        );
        assert_eq!(
            ServiceError::QueueError("queue issue".into()).response_message(),
            "Internal server error"
        );

        // User-facing errors SHOULD include the actual message
        assert_eq!(
            ServiceError::NotFound("Order not found".into()).response_message(),
            "Not found: Order not found"
        );
        assert_eq!(
            ServiceError::ValidationError("Invalid email".into()).response_message(),
            "Validation error: Invalid email"
        );
    }

    #[test]
    fn api_error_delegates_to_service_error_status() {
        let service_err = ServiceError::NotFound("test".into());

        // ApiError should use the same status code as ServiceError
        let status = service_err.status_code();
        let api_err = ApiError::ServiceError(service_err);

        let api_status = match &api_err {
            ApiError::ServiceError(se) => se.status_code(),
            _ => panic!("Expected ServiceError variant"),
        };
        assert_eq!(status, api_status);
        assert_eq!(api_status, StatusCode::NOT_FOUND);
    }

    #[test]
    fn acp_error_response_from_service_error() {
        let validation_err = ServiceError::ValidationError("field required".into());
        let acp_response = ACPErrorResponse::from(&validation_err);

        assert_eq!(acp_response.error.code, "validation_error");
        assert!(matches!(
            acp_response.error.error_type,
            ACPErrorType::InvalidRequestError
        ));

        let auth_err = ServiceError::Unauthorized("invalid token".into());
        let acp_response = ACPErrorResponse::from(&auth_err);
        assert!(matches!(
            acp_response.error.error_type,
            ACPErrorType::AuthenticationError
        ));

        let rate_err = ServiceError::RateLimitExceeded;
        let acp_response = ACPErrorResponse::from(&rate_err);
        assert!(matches!(
            acp_response.error.error_type,
            ACPErrorType::RateLimitError
        ));
    }
}
