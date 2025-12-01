use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

/// Ledger entry types for accounting
#[derive(Debug, Clone, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(32))")]
pub enum LedgerEntryType {
    #[sea_orm(string_value = "Debit")]
    Debit,
    #[sea_orm(string_value = "Credit")]
    Credit,
}

/// Ledger entry status
#[derive(Debug, Clone, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(32))")]
pub enum LedgerEntryStatus {
    #[sea_orm(string_value = "Pending")]
    Pending,
    #[sea_orm(string_value = "Posted")]
    Posted,
    #[sea_orm(string_value = "Reversed")]
    Reversed,
}

/// Ledger entry for double-entry bookkeeping
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "ledger_entries")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,

    /// Transaction ID to group related entries
    pub transaction_id: Uuid,

    /// Account code/name (e.g., "Cash", "Revenue", "Accounts Receivable")
    pub account: String,

    /// Entry type (Debit or Credit)
    pub entry_type: LedgerEntryType,

    /// Amount in the transaction currency
    pub amount: Decimal,

    /// Currency code (e.g., "USD", "EUR")
    pub currency: String,

    /// Description of the transaction
    pub description: String,

    /// Reference to related entity (e.g., order_id, invoice_id)
    pub reference_id: Option<Uuid>,

    /// Reference type (e.g., "Order", "Invoice", "Payment")
    pub reference_type: Option<String>,

    /// Entry status
    pub status: LedgerEntryStatus,

    /// Posting date (when the entry is officially recorded)
    pub posting_date: DateTime<Utc>,

    /// Metadata for additional information
    pub metadata: Option<Json>,

    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,

    /// Optional: for user audit trail
    pub created_by: Option<Uuid>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

impl Model {
    /// Check if this is a debit entry
    pub fn is_debit(&self) -> bool {
        self.entry_type == LedgerEntryType::Debit
    }

    /// Check if this is a credit entry
    pub fn is_credit(&self) -> bool {
        self.entry_type == LedgerEntryType::Credit
    }

    /// Get signed amount (positive for debit, negative for credit)
    pub fn signed_amount(&self) -> Decimal {
        match self.entry_type {
            LedgerEntryType::Debit => self.amount,
            LedgerEntryType::Credit => -self.amount,
        }
    }
}
