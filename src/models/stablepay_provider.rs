use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize, Validate)]
#[sea_orm(table_name = "stablepay_providers")]
pub struct Model {
    #[sea_orm(primary_key, column_type = "Uuid")]
    pub id: Uuid,
    pub name: String,
    pub provider_type: String,
    pub api_key_encrypted: Option<String>,
    pub webhook_secret_encrypted: Option<String>,
    #[sea_orm(column_type = "Json")]
    pub configuration: Option<serde_json::Value>,
    #[sea_orm(column_type = "Decimal(Some((5, 4)))")]
    pub fee_percentage: Decimal,
    #[sea_orm(column_type = "Decimal(Some((10, 4)))")]
    pub fee_fixed: Decimal,
    #[sea_orm(column_type = "Json")]
    pub supported_currencies: Vec<String>,
    #[sea_orm(column_type = "Json")]
    pub supported_countries: Vec<String>,
    pub is_active: bool,
    pub priority: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ProviderType {
    Stripe,
    PayPal,
    BankTransfer,
    Crypto,
    Direct,
    Custom(String),
}

impl std::fmt::Display for ProviderType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Stripe => write!(f, "stripe"),
            Self::PayPal => write!(f, "paypal"),
            Self::BankTransfer => write!(f, "bank_transfer"),
            Self::Crypto => write!(f, "crypto"),
            Self::Direct => write!(f, "direct"),
            Self::Custom(s) => write!(f, "{}", s),
        }
    }
}

impl Model {
    /// Calculate total fee for an amount
    pub fn calculate_fee(&self, amount: Decimal) -> Decimal {
        (amount * self.fee_percentage) + self.fee_fixed
    }

    /// Calculate net amount after fees
    pub fn calculate_net_amount(&self, amount: Decimal) -> Decimal {
        amount - self.calculate_fee(amount)
    }

    /// Check if provider supports currency
    pub fn supports_currency(&self, currency: &str) -> bool {
        self.supported_currencies.iter().any(|c| c == currency)
    }

    /// Check if provider supports country
    pub fn supports_country(&self, country: &str) -> bool {
        self.supported_countries.iter().any(|c| c == country)
    }
}
