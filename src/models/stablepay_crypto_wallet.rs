use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize, Validate)]
#[sea_orm(table_name = "stablepay_crypto_wallets")]
pub struct Model {
    #[sea_orm(primary_key, column_type = "Uuid")]
    pub id: Uuid,
    pub customer_id: Uuid,
    pub wallet_address: String,
    pub blockchain: String,
    pub wallet_type: String,
    pub label: Option<String>,
    pub is_verified: bool,
    pub is_default: bool,
    pub last_used_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum WalletType {
    Hot,
    Cold,
    Custodial,
    NonCustodial,
}

impl std::fmt::Display for WalletType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Hot => write!(f, "hot"),
            Self::Cold => write!(f, "cold"),
            Self::Custodial => write!(f, "custodial"),
            Self::NonCustodial => write!(f, "non_custodial"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Blockchain {
    Ethereum,
    Polygon,
    Arbitrum,
    Optimism,
    Base,
    Solana,
}

impl std::fmt::Display for Blockchain {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Ethereum => write!(f, "ethereum"),
            Self::Polygon => write!(f, "polygon"),
            Self::Arbitrum => write!(f, "arbitrum"),
            Self::Optimism => write!(f, "optimism"),
            Self::Base => write!(f, "base"),
            Self::Solana => write!(f, "solana"),
        }
    }
}

impl Model {
    /// Get shortened wallet address for display
    pub fn short_address(&self) -> String {
        if self.wallet_address.len() > 10 {
            format!(
                "{}...{}",
                &self.wallet_address[..6],
                &self.wallet_address[self.wallet_address.len() - 4..]
            )
        } else {
            self.wallet_address.clone()
        }
    }

    /// Check if wallet address is valid format
    pub fn is_valid_address(&self) -> bool {
        match self.blockchain.as_str() {
            "ethereum" | "polygon" | "arbitrum" | "optimism" | "base" => {
                // EVM addresses start with 0x and are 42 characters
                self.wallet_address.starts_with("0x") && self.wallet_address.len() == 42
            }
            "solana" => {
                // Solana addresses are base58 and typically 32-44 characters
                self.wallet_address.len() >= 32 && self.wallet_address.len() <= 44
            }
            _ => false,
        }
    }
}
