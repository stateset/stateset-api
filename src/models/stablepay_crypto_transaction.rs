use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize, Validate)]
#[sea_orm(table_name = "stablepay_crypto_transactions")]
pub struct Model {
    #[sea_orm(primary_key, column_type = "Uuid")]
    pub id: Uuid,
    pub transaction_id: Uuid,

    // Blockchain details
    pub blockchain: String,
    pub network: String,
    pub token_contract_address: String,
    pub token_symbol: String,
    pub token_decimals: i32,

    // Transaction details
    pub tx_hash: Option<String>,
    pub block_number: Option<i64>,
    pub block_timestamp: Option<DateTime<Utc>>,

    // Addresses
    pub from_address: String,
    pub to_address: String,

    // Amounts
    pub amount_raw: String,
    #[sea_orm(column_type = "Decimal(Some((19, 6)))")]
    pub amount_decimal: Decimal,

    // Gas fees
    #[sea_orm(column_type = "Decimal(Some((19, 9)))")]
    pub gas_price_gwei: Option<Decimal>,
    pub gas_used: Option<i64>,
    #[sea_orm(column_type = "Decimal(Some((19, 9)))")]
    pub gas_cost_native: Option<Decimal>,
    #[sea_orm(column_type = "Decimal(Some((19, 4)))")]
    pub gas_cost_usd: Option<Decimal>,

    // Status
    pub status: String,
    pub confirmations: i32,
    pub required_confirmations: i32,

    // Error handling
    pub error_code: Option<String>,
    pub error_message: Option<String>,

    // Metadata
    pub nonce: Option<i64>,
    pub input_data: Option<String>,
    #[sea_orm(column_type = "Json")]
    pub metadata: Option<serde_json::Value>,

    pub created_at: DateTime<Utc>,
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CryptoTransactionStatus {
    Pending,
    Confirming,
    Confirmed,
    Failed,
}

impl std::fmt::Display for CryptoTransactionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending => write!(f, "pending"),
            Self::Confirming => write!(f, "confirming"),
            Self::Confirmed => write!(f, "confirmed"),
            Self::Failed => write!(f, "failed"),
        }
    }
}

impl Model {
    /// Check if transaction is confirmed
    pub fn is_confirmed(&self) -> bool {
        self.confirmations >= self.required_confirmations
    }

    /// Get confirmation progress percentage
    pub fn confirmation_progress(&self) -> Decimal {
        if self.required_confirmations == 0 {
            return Decimal::from(100);
        }
        let progress =
            (self.confirmations as f64 / self.required_confirmations as f64 * 100.0).min(100.0);
        Decimal::try_from(progress).unwrap_or(Decimal::ZERO)
    }

    /// Get total transaction cost in USD (amount + gas)
    pub fn total_cost_usd(&self) -> Decimal {
        self.amount_decimal + self.gas_cost_usd.unwrap_or(Decimal::ZERO)
    }

    /// Get explorer URL for transaction
    pub fn explorer_url(&self, base_url: &str) -> Option<String> {
        self.tx_hash
            .as_ref()
            .map(|hash| format!("{}/tx/{}", base_url, hash))
    }

    /// Get short transaction hash
    pub fn short_tx_hash(&self) -> Option<String> {
        self.tx_hash.as_ref().map(|hash| {
            if hash.len() > 10 {
                format!("{}...{}", &hash[..6], &hash[hash.len() - 4..])
            } else {
                hash.clone()
            }
        })
    }
}
