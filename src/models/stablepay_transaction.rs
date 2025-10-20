use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize, Validate)]
#[sea_orm(table_name = "stablepay_transactions")]
pub struct Model {
    #[sea_orm(primary_key, column_type = "Uuid")]
    pub id: Uuid,
    pub transaction_number: String,

    // Relationships
    pub order_id: Option<Uuid>,
    pub customer_id: Uuid,
    pub payment_method_id: Option<Uuid>,
    pub provider_id: Uuid,

    // Amount details
    #[sea_orm(column_type = "Decimal(Some((19, 4)))")]
    pub amount: Decimal,
    pub currency: String,
    #[sea_orm(column_type = "Decimal(Some((19, 4)))")]
    pub original_amount: Option<Decimal>,
    pub original_currency: Option<String>,
    #[sea_orm(column_type = "Decimal(Some((12, 6)))")]
    pub exchange_rate: Option<Decimal>,

    // Fee breakdown
    #[sea_orm(column_type = "Decimal(Some((19, 4)))")]
    pub provider_fee: Decimal,
    #[sea_orm(column_type = "Decimal(Some((19, 4)))")]
    pub platform_fee: Decimal,
    #[sea_orm(column_type = "Decimal(Some((19, 4)))")]
    pub total_fees: Decimal,
    #[sea_orm(column_type = "Decimal(Some((19, 4)))")]
    pub net_amount: Decimal,

    // Status and processing
    pub status: String,
    pub payment_intent_id: Option<String>,
    pub charge_id: Option<String>,

    // Timing
    pub initiated_at: DateTime<Utc>,
    pub processed_at: Option<DateTime<Utc>>,
    pub settled_at: Option<DateTime<Utc>>,
    pub estimated_settlement_date: Option<NaiveDate>,

    // Error handling
    pub failure_code: Option<String>,
    pub failure_message: Option<String>,
    pub retry_count: i32,

    // Reconciliation
    pub is_reconciled: bool,
    pub reconciled_at: Option<DateTime<Utc>>,
    pub reconciliation_id: Option<Uuid>,

    // Security
    #[sea_orm(column_type = "Decimal(Some((5, 2)))")]
    pub risk_score: Option<Decimal>,
    pub is_flagged_for_review: bool,
    #[sea_orm(column_type = "Json")]
    pub fraud_indicators: Option<serde_json::Value>,

    // Additional data
    pub description: Option<String>,
    #[sea_orm(column_type = "Json")]
    pub metadata: Option<serde_json::Value>,
    #[sea_orm(column_type = "Json")]
    pub gateway_response: Option<serde_json::Value>,

    // Audit
    pub created_at: DateTime<Utc>,
    pub updated_at: Option<DateTime<Utc>>,
    pub created_by: Option<Uuid>,

    // Idempotency
    pub idempotency_key: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TransactionStatus {
    Pending,
    Processing,
    Succeeded,
    Failed,
    Cancelled,
    Refunded,
    PartiallyRefunded,
}

impl std::fmt::Display for TransactionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending => write!(f, "pending"),
            Self::Processing => write!(f, "processing"),
            Self::Succeeded => write!(f, "succeeded"),
            Self::Failed => write!(f, "failed"),
            Self::Cancelled => write!(f, "cancelled"),
            Self::Refunded => write!(f, "refunded"),
            Self::PartiallyRefunded => write!(f, "partially_refunded"),
        }
    }
}

impl Model {
    /// Calculate the effective fee rate for this transaction
    pub fn effective_fee_rate(&self) -> Decimal {
        if self.amount > Decimal::ZERO {
            self.total_fees / self.amount
        } else {
            Decimal::ZERO
        }
    }

    /// Check if transaction is settled
    pub fn is_settled(&self) -> bool {
        self.settled_at.is_some()
    }

    /// Get settlement delay in hours
    pub fn settlement_delay_hours(&self) -> Option<i64> {
        match (self.processed_at, self.settled_at) {
            (Some(processed), Some(settled)) => Some((settled - processed).num_hours()),
            _ => None,
        }
    }
}
