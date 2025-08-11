use thiserror::Error;
use uuid::Uuid;

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

fn main() {
    let error1 = InventoryError::NotFound("Test product".to_string());
    println!("Error: {}", error1);
    println!("Error type: {}", error1.error_type());

    let error2 = InventoryError::InsufficientInventory(Uuid::new_v4());
    println!("Error: {}", error2);
    println!("Error type: {}", error2.error_type());

    let error3 = InventoryError::ConcurrentModification(Uuid::new_v4());
    println!("Error: {}", error3);
    println!("Error type: {}", error3.error_type());

    println!("The error types are working as expected!");
}
