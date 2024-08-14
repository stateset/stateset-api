use serde::{Serialize, Deserialize};
use validator::{Validate, ValidationError};
use diesel::prelude::*;
use diesel::sql_types::Text;
use chrono::{NaiveDateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize, Queryable, Insertable, AsChangeset, Validate)]
#[table_name = "returns"]
pub struct Return {
    pub id: i32,
    #[validate(range(min = 1, message = "Order ID must be positive"))]
    pub order_id: i32,
    #[validate(range(min = 1, message = "Customer ID must be positive"))]
    pub customer_id: i32,
    pub status: ReturnStatus,
    #[validate(length(min = 1, max = 500, message = "Reason must be between 1 and 500 characters"))]
    pub reason: String,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, AsExpression, FromSqlRow)]
#[sql_type = "Text"]
pub enum ReturnStatus {
    Requested,
    Approved,
    Rejected,
    Received,
    Refunded,
}

#[derive(Debug, Clone, Serialize, Deserialize, Associations, Queryable, Insertable, Validate)]
#[belongs_to(Return)]
#[belongs_to(Product)]
#[table_name = "return_items"]
pub struct ReturnItem {
    pub id: i32,
    #[validate(range(min = 1, message = "Return ID must be positive"))]
    pub return_id: i32,
    #[validate(range(min = 1, message = "Product ID must be positive"))]
    pub product_id: i32,
    #[validate(range(min = 1, message = "Quantity must be at least 1"))]
    pub quantity: i32,
    #[validate(length(min = 1, max = 255, message = "Reason must be between 1 and 255 characters"))]
    pub reason: String,
}

impl Return {
    pub fn new(order_id: i32, customer_id: i32, reason: String) -> Result<Self, ValidationError> {
        let now = Utc::now().naive_utc();
        let return_request = Self {
            id: 0, // Assuming database will auto-increment this
            order_id,
            customer_id,
            status: ReturnStatus::Requested,
            reason,
            created_at: now,
            updated_at: now,
        };
        return_request.validate()?;
        Ok(return_request)
    }

    pub fn update_status(&mut self, new_status: ReturnStatus) -> Result<(), String> {
        if self.status == ReturnStatus::Refunded {
            return Err("Cannot update status of a refunded return".into());
        }
        self.status = new_status;
        self.updated_at = Utc::now().naive_utc();
        Ok(())
    }
}

impl ReturnStatus {
    pub fn is_final(&self) -> bool {
        matches!(self, ReturnStatus::Refunded | ReturnStatus::Rejected)
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            ReturnStatus::Requested => "Requested",
            ReturnStatus::Approved => "Approved",
            ReturnStatus::Rejected => "Rejected",
            ReturnStatus::Received => "Received",
            ReturnStatus::Refunded => "Refunded",
        }
    }
}

impl ReturnItem {
    pub fn new(return_id: i32, product_id: i32, quantity: i32, reason: String) -> Result<Self, ValidationError> {
        let item = Self {
            id: 0, // Assuming database will auto-increment this
            return_id,
            product_id,
            quantity,
            reason,
        };
        item.validate()?;
        Ok(item)
    }
}

// Implement a custom validator for ReturnStatus if needed
fn validate_return_status(status: &ReturnStatus) -> Result<(), ValidationError> {
    match status {
        ReturnStatus::Requested | ReturnStatus::Approved | ReturnStatus::Rejected | ReturnStatus::Received | ReturnStatus::Refunded => Ok(()),
        _ => Err(ValidationError::new("Invalid return status")),
    }
}