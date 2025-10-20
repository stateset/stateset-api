use tonic::Status;
use tracing::error;

use super::ServiceError;

/// Extension trait for converting ServiceError to gRPC Status with proper codes
pub trait IntoGrpcStatus {
    fn into_grpc_status(self) -> Status;
}

impl IntoGrpcStatus for ServiceError {
    fn into_grpc_status(self) -> Status {
        match self {
            ServiceError::NotFound(msg) | ServiceError::NotFoundError(msg) => {
                Status::not_found(msg)
            }
            ServiceError::ValidationError(msg)
            | ServiceError::InvalidStatus(msg)
            | ServiceError::InvalidInput(msg) => Status::invalid_argument(msg),
            ServiceError::AuthError(msg)
            | ServiceError::JwtError(msg)
            | ServiceError::Unauthorized(msg) => Status::unauthenticated(msg),
            ServiceError::Forbidden(msg) => Status::permission_denied(msg),
            ServiceError::Conflict(msg) => Status::already_exists(msg),
            ServiceError::InsufficientStock(msg) => {
                Status::failed_precondition(format!("Insufficient stock: {}", msg))
            }
            ServiceError::PaymentFailed(msg) => {
                Status::failed_precondition(format!("Payment failed: {}", msg))
            }
            ServiceError::ExternalApiError(msg) | ServiceError::ExternalServiceError(msg) => {
                error!("External service error: {}", msg);
                Status::unavailable(format!("External service unavailable: {}", msg))
            }
            ServiceError::DatabaseError(err) => {
                error!("Database error: {}", err);
                Status::internal("Database operation failed")
            }
            ServiceError::CacheError(msg) => {
                error!("Cache error: {}", msg);
                // Cache errors shouldn't fail requests, just log and continue
                Status::internal("Cache operation failed")
            }
            ServiceError::QueueError(msg) => {
                error!("Queue error: {}", msg);
                Status::internal("Message queue operation failed")
            }
            ServiceError::SerializationError(msg) => {
                error!("Serialization error: {}", msg);
                Status::internal("Data serialization failed")
            }
            ServiceError::InternalError(msg) | ServiceError::HashError(msg) => {
                error!("Internal error: {}", msg);
                Status::internal("Internal server error")
            }
            ServiceError::BadRequest(msg) | ServiceError::InvalidOperation(msg) => {
                Status::invalid_argument(msg)
            }
            ServiceError::RateLimitExceeded => Status::resource_exhausted("Rate limit exceeded"),
            ServiceError::CircuitBreakerOpen => {
                Status::unavailable("Service temporarily unavailable")
            }
            ServiceError::MigrationError(msg) => {
                error!("Migration error: {}", msg);
                Status::internal("Database migration failed")
            }
            ServiceError::ConcurrentModification(id) => Status::aborted(format!(
                "Concurrent modification detected for resource: {}",
                id
            )),
            ServiceError::EventError(msg) => {
                error!("Event processing error: {}", msg);
                Status::internal("Event processing failed")
            }
            ServiceError::OrderError(msg) => {
                Status::failed_precondition(format!("Order error: {}", msg))
            }
            ServiceError::InventoryError(msg) => {
                Status::failed_precondition(format!("Inventory error: {}", msg))
            }
            ServiceError::InternalServerError => Status::internal("Internal server error"),
            ServiceError::Other(err) => {
                error!("Other error: {}", err);
                Status::internal("An unexpected error occurred")
            }
        }
    }
}

/// Helper function to map Result<T, ServiceError> to Result<T, Status>
pub fn map_service_error<T>(result: Result<T, ServiceError>) -> Result<T, Status> {
    result.map_err(|e| e.into_grpc_status())
}
