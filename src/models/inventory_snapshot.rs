use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use validator::Validate;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize, Validate)]
#[sea_orm(table_name = "inventory_snapshots")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub date: DateTime<Utc>,
    pub total_value: Decimal,
    pub total_units: i32,
    pub product_id: Option<String>,
    pub warehouse_id: Option<i32>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

// Convenient type aliases
pub type InventorySnapshotModel = Model;
pub type NewInventorySnapshot = ActiveModel;
