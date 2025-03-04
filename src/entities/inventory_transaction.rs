use sea_orm::entity::prelude::*;
use chrono::{DateTime, Utc};
use uuid::Uuid;
use serde::{Serialize, Deserialize};

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

impl ActiveModelBehavior for ActiveModel {
    // Add business logic here
    fn before_save(mut self, _insert: bool) -> Result<Self, DbErr> {
        // Calculate the new quantity on insert
        if let (sea_orm::ActiveValue::Set(prev_qty), sea_orm::ActiveValue::Set(qty)) = 
            (&self.previous_quantity, &self.quantity) 
        {
            let new_qty = prev_qty + qty;
            self.new_quantity = Set(new_qty);
        }
        
        // Validate the transaction type
        if let sea_orm::ActiveValue::Set(type_str) = &self.r#type {
            if TransactionType::from_str(type_str).is_none() {
                return Err(DbErr::Custom(format!("Invalid transaction type: {}", type_str)));
            }
        }
        
        Ok(self)
    }
}