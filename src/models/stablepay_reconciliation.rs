use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize, Validate)]
#[sea_orm(table_name = "stablepay_reconciliations")]
pub struct Model {
    #[sea_orm(primary_key, column_type = "Uuid")]
    pub id: Uuid,
    pub reconciliation_number: String,

    // Period
    pub period_start: NaiveDate,
    pub period_end: NaiveDate,

    // Provider
    pub provider_id: Uuid,

    // Summary
    pub total_transactions: i32,
    #[sea_orm(column_type = "Decimal(Some((19, 4)))")]
    pub total_amount: Decimal,
    #[sea_orm(column_type = "Decimal(Some((19, 4)))")]
    pub total_fees: Decimal,
    pub matched_transactions: i32,
    pub unmatched_transactions: i32,

    // Discrepancies
    #[sea_orm(column_type = "Decimal(Some((19, 4)))")]
    pub discrepancy_amount: Decimal,
    pub discrepancy_count: i32,

    // Status
    pub status: String,

    // Files
    pub provider_statement_url: Option<String>,
    pub reconciliation_report_url: Option<String>,

    // Timing
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,

    // Additional
    #[sea_orm(column_type = "Json")]
    pub metadata: Option<serde_json::Value>,
    pub notes: Option<String>,

    pub created_at: DateTime<Utc>,
    pub updated_at: Option<DateTime<Utc>>,
    pub created_by: Option<Uuid>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ReconciliationStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
    RequiresReview,
}

impl std::fmt::Display for ReconciliationStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending => write!(f, "pending"),
            Self::InProgress => write!(f, "in_progress"),
            Self::Completed => write!(f, "completed"),
            Self::Failed => write!(f, "failed"),
            Self::RequiresReview => write!(f, "requires_review"),
        }
    }
}

impl Model {
    /// Calculate match rate percentage
    pub fn match_rate(&self) -> Decimal {
        if self.total_transactions > 0 {
            Decimal::from(self.matched_transactions) / Decimal::from(self.total_transactions)
                * Decimal::from(100)
        } else {
            Decimal::ZERO
        }
    }

    /// Check if reconciliation needs review
    pub fn needs_review(&self) -> bool {
        self.unmatched_transactions > 0 || self.discrepancy_count > 0
    }

    /// Calculate average fee percentage
    pub fn average_fee_percentage(&self) -> Decimal {
        if self.total_amount > Decimal::ZERO {
            (self.total_fees / self.total_amount) * Decimal::from(100)
        } else {
            Decimal::ZERO
        }
    }
}

pub mod stablepay_reconciliation_items {
    use super::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize, Validate)]
    #[sea_orm(table_name = "stablepay_reconciliation_items")]
    pub struct Model {
        #[sea_orm(primary_key, column_type = "Uuid")]
        pub id: Uuid,
        pub reconciliation_id: Uuid,
        pub transaction_id: Option<Uuid>,

        // External data
        pub external_transaction_id: Option<String>,
        #[sea_orm(column_type = "Decimal(Some((19, 4)))")]
        pub external_amount: Option<Decimal>,
        pub external_currency: Option<String>,
        pub external_date: Option<DateTime<Utc>>,

        // Matching
        pub match_status: String,
        #[sea_orm(column_type = "Decimal(Some((5, 2)))")]
        pub match_score: Option<Decimal>,

        // Discrepancy
        #[sea_orm(column_type = "Decimal(Some((19, 4)))")]
        pub amount_difference: Option<Decimal>,
        pub discrepancy_reason: Option<String>,

        // Resolution
        pub is_resolved: bool,
        pub resolved_at: Option<DateTime<Utc>>,
        pub resolved_by: Option<Uuid>,
        pub resolution_notes: Option<String>,

        pub created_at: DateTime<Utc>,
        pub updated_at: Option<DateTime<Utc>>,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

pub use stablepay_reconciliation_items::Model as ReconciliationItem;
