use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize, Validate)]
#[sea_orm(table_name = "stablepay_refunds")]
pub struct Model {
    #[sea_orm(primary_key, column_type = "Uuid")]
    pub id: Uuid,
    pub refund_number: String,
    pub transaction_id: Uuid,

    // Amount
    #[sea_orm(column_type = "Decimal(Some((19, 4)))")]
    pub amount: Decimal,
    pub currency: String,

    // Fees
    #[sea_orm(column_type = "Decimal(Some((19, 4)))")]
    pub refunded_fees: Decimal,
    #[sea_orm(column_type = "Decimal(Some((19, 4)))")]
    pub net_refund: Decimal,

    // Status
    pub status: String,
    pub refund_id_external: Option<String>,

    // Reason
    pub reason: Option<String>,
    pub reason_detail: Option<String>,

    // Timing
    pub requested_at: DateTime<Utc>,
    pub processed_at: Option<DateTime<Utc>>,

    // Error
    pub failure_code: Option<String>,
    pub failure_message: Option<String>,

    // Additional
    #[sea_orm(column_type = "Json")]
    pub metadata: Option<serde_json::Value>,
    #[sea_orm(column_type = "Json")]
    pub gateway_response: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
    pub updated_at: Option<DateTime<Utc>>,
    pub created_by: Option<Uuid>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum RefundStatus {
    Pending,
    Processing,
    Succeeded,
    Failed,
    Cancelled,
}

impl std::fmt::Display for RefundStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending => write!(f, "pending"),
            Self::Processing => write!(f, "processing"),
            Self::Succeeded => write!(f, "succeeded"),
            Self::Failed => write!(f, "failed"),
            Self::Cancelled => write!(f, "cancelled"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum RefundReason {
    Duplicate,
    Fraudulent,
    RequestedByCustomer,
    ProductIssue,
    ServiceNotProvided,
    Other,
}

impl std::fmt::Display for RefundReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Duplicate => write!(f, "duplicate"),
            Self::Fraudulent => write!(f, "fraudulent"),
            Self::RequestedByCustomer => write!(f, "requested_by_customer"),
            Self::ProductIssue => write!(f, "product_issue"),
            Self::ServiceNotProvided => write!(f, "service_not_provided"),
            Self::Other => write!(f, "other"),
        }
    }
}

impl Model {
    /// Get refund percentage of original amount
    pub fn refund_percentage(&self, original_amount: Decimal) -> Decimal {
        if original_amount > Decimal::ZERO {
            (self.amount / original_amount) * Decimal::from(100)
        } else {
            Decimal::ZERO
        }
    }

    /// Check if this is a full refund
    pub fn is_full_refund(&self, original_amount: Decimal) -> bool {
        self.amount >= original_amount
    }
}
