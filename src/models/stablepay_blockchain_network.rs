use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize, Validate)]
#[sea_orm(table_name = "stablepay_blockchain_networks")]
pub struct Model {
    #[sea_orm(primary_key, column_type = "Uuid")]
    pub id: Uuid,
    pub blockchain: String,
    pub network: String,

    // Network details
    pub chain_id: Option<i32>,
    pub rpc_url: String,
    pub explorer_url: Option<String>,

    // Gas configuration
    #[sea_orm(column_type = "Decimal(Some((19, 9)))")]
    pub average_gas_price_gwei: Option<Decimal>,
    #[sea_orm(column_type = "Decimal(Some((19, 9)))")]
    pub fast_gas_price_gwei: Option<Decimal>,
    pub native_token_symbol: String,

    // Status
    pub is_active: bool,
    pub is_testnet: bool,

    // Performance
    pub average_block_time_seconds: Option<i32>,
    pub average_confirmation_time_seconds: Option<i32>,

    pub created_at: DateTime<Utc>,
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

impl Model {
    /// Get full network name (e.g., "Ethereum Mainnet")
    pub fn full_name(&self) -> String {
        format!("{} {}", self.blockchain, self.network)
            .split_whitespace()
            .map(|word| {
                let mut chars = word.chars();
                match chars.next() {
                    None => String::new(),
                    Some(f) => f.to_uppercase().collect::<String>() + chars.as_str(),
                }
            })
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Estimate gas cost in USD
    pub fn estimate_gas_cost_usd(
        &self,
        gas_units: i64,
        native_token_price_usd: Decimal,
    ) -> Option<Decimal> {
        self.average_gas_price_gwei.map(|gas_price| {
            // Convert gas price from gwei to native token
            let gas_price_native = gas_price / Decimal::from(1_000_000_000); // 1 gwei = 10^-9 native token
            let gas_cost_native = gas_price_native * Decimal::from(gas_units);
            gas_cost_native * native_token_price_usd
        })
    }

    /// Get estimated confirmation time in minutes
    pub fn estimated_confirmation_time_minutes(&self) -> Option<i32> {
        self.average_confirmation_time_seconds
            .map(|seconds| seconds / 60)
    }

    /// Check if this is a Layer 2 network
    pub fn is_layer2(&self) -> bool {
        matches!(
            self.blockchain.as_str(),
            "polygon" | "arbitrum" | "optimism" | "base"
        )
    }
}

pub mod stablepay_token_contracts {
    use super::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize, Validate)]
    #[sea_orm(table_name = "stablepay_token_contracts")]
    pub struct Model {
        #[sea_orm(primary_key, column_type = "Uuid")]
        pub id: Uuid,
        pub network_id: Uuid,

        // Token details
        pub token_symbol: String,
        pub token_name: String,
        pub contract_address: String,
        pub decimals: i32,

        // Contract info
        pub token_standard: Option<String>,
        pub is_native: bool,

        // Features
        pub supports_permit: bool,
        pub supports_meta_transactions: bool,

        // Status
        pub is_active: bool,

        // Metadata
        pub logo_url: Option<String>,
        pub website: Option<String>,
        #[sea_orm(column_type = "Json")]
        pub metadata: Option<serde_json::Value>,

        pub created_at: DateTime<Utc>,
        pub updated_at: Option<DateTime<Utc>>,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

pub use stablepay_token_contracts::Model as TokenContract;

impl TokenContract {
    /// Convert human-readable amount to smallest unit (raw amount)
    pub fn to_raw_amount(&self, amount: Decimal) -> String {
        let multiplier = Decimal::from(10_i64.pow(self.decimals as u32));
        let raw = amount * multiplier;
        format!("{:.0}", raw)
    }

    /// Convert raw amount to human-readable decimal
    pub fn to_decimal_amount(&self, raw_amount: &str) -> Option<Decimal> {
        let raw = Decimal::from_str_exact(raw_amount).ok()?;
        let divisor = Decimal::from(10_i64.pow(self.decimals as u32));
        Some(raw / divisor)
    }

    /// Check if this is a stablecoin
    pub fn is_stablecoin(&self) -> bool {
        matches!(self.token_symbol.as_str(), "USDC" | "USDT" | "DAI" | "BUSD")
    }
}
