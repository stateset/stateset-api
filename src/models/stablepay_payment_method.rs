use chrono::{DateTime, Datelike, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize, Validate)]
#[sea_orm(table_name = "stablepay_payment_methods")]
pub struct Model {
    #[sea_orm(primary_key, column_type = "Uuid")]
    pub id: Uuid,
    pub customer_id: Uuid,
    pub provider_id: Uuid,
    pub external_id: Option<String>,
    pub method_type: String,
    pub brand: Option<String>,
    pub last_four: Option<String>,
    pub exp_month: Option<i32>,
    pub exp_year: Option<i32>,
    pub holder_name: Option<String>,
    #[sea_orm(column_type = "Json")]
    pub billing_address: Option<serde_json::Value>,
    pub is_default: bool,
    pub is_verified: bool,
    #[sea_orm(column_type = "Json")]
    pub metadata: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PaymentMethodType {
    Card,
    BankAccount,
    CryptoWallet,
    PayPal,
    ApplePay,
    GooglePay,
}

impl std::fmt::Display for PaymentMethodType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Card => write!(f, "card"),
            Self::BankAccount => write!(f, "bank_account"),
            Self::CryptoWallet => write!(f, "crypto_wallet"),
            Self::PayPal => write!(f, "paypal"),
            Self::ApplePay => write!(f, "apple_pay"),
            Self::GooglePay => write!(f, "google_pay"),
        }
    }
}

impl Model {
    /// Check if payment method is expired
    pub fn is_expired(&self) -> bool {
        if let (Some(month), Some(year)) = (self.exp_month, self.exp_year) {
            let now = Utc::now();
            let current_year = now.year();
            let current_month = now.month() as i32;

            year < current_year || (year == current_year && month < current_month)
        } else {
            false
        }
    }

    /// Get masked card number
    pub fn masked_number(&self) -> Option<String> {
        self.last_four.as_ref().map(|last| format!("•••• {}", last))
    }
}
