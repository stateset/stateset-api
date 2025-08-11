use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(32))")]
pub enum ReturnItemStatus {
    #[sea_orm(string_value = "Pending")]
    Pending,
    #[sea_orm(string_value = "Approved")]
    Approved,
    #[sea_orm(string_value = "Rejected")]
    Rejected,
    #[sea_orm(string_value = "Received")]
    Received,
    #[sea_orm(string_value = "Processed")]
    Processed,
}

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "return_items")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub return_id: Uuid,
    pub order_item_id: Option<i32>,
    pub sku: String,
    pub product_name: String,
    pub quantity: i32,
    pub reason: String,
    pub condition_received: Option<String>,
    pub unit_price: Decimal,
    pub total_refund_amount: Decimal,
    pub status: ReturnItemStatus,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "crate::models::return_entity::Entity",
        from = "Column::ReturnId",
        to = "crate::models::return_entity::Column::Id"
    )]
    Return,
}

impl Related<crate::models::return_entity::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Return.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
