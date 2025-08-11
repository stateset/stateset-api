use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sea_orm::entity::prelude::*;
use sea_orm::{ActiveModelBehavior, ActiveValue, Set};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Types of inventory transactions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransactionType {
    Receive,
    Ship,
    Return,
    Adjust,
    Count,
    Transfer,
    Allocate,
    Deallocate,
    Reserve,
    Release,
    Production,
    Move,
}

impl TransactionType {
    pub fn as_str(&self) -> &'static str {
        match self {
            TransactionType::Receive => "receive",
            TransactionType::Ship => "ship",
            TransactionType::Return => "return",
            TransactionType::Adjust => "adjust",
            TransactionType::Count => "count",
            TransactionType::Transfer => "transfer",
            TransactionType::Allocate => "allocate",
            TransactionType::Deallocate => "deallocate",
            TransactionType::Reserve => "reserve",
            TransactionType::Release => "release",
            TransactionType::Production => "production",
            TransactionType::Move => "move",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "receive" => Some(TransactionType::Receive),
            "ship" => Some(TransactionType::Ship),
            "return" => Some(TransactionType::Return),
            "adjust" => Some(TransactionType::Adjust),
            "count" => Some(TransactionType::Count),
            "transfer" => Some(TransactionType::Transfer),
            "allocate" => Some(TransactionType::Allocate),
            "deallocate" => Some(TransactionType::Deallocate),
            "reserve" => Some(TransactionType::Reserve),
            "release" => Some(TransactionType::Release),
            "production" => Some(TransactionType::Production),
            "move" => Some(TransactionType::Move),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "inventory_transactions")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub product_id: Uuid,
    pub location_id: Uuid,
    pub r#type: String, // Storing as string in DB, but will convert to/from enum
    pub quantity: i32,
    pub previous_quantity: i32,
    pub new_quantity: i32,
    pub reference_id: Option<Uuid>,
    pub reference_type: Option<String>,
    pub reason: Option<String>,
    pub notes: Option<String>,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
}

// No relations for inventory transactions yet

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

#[async_trait]
impl ActiveModelBehavior for ActiveModel {
    async fn before_save<C: ConnectionTrait>(self, _db: &C, _insert: bool) -> Result<Self, DbErr>
    where
        C: ConnectionTrait,
    {
        let mut active_model = self;
        if let ActiveValue::NotSet = active_model.created_at {
            active_model.created_at = Set(Utc::now());
        }
        Ok(active_model)
    }
}
