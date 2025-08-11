use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize, Validate)]
#[sea_orm(table_name = "item_receipts")]
pub struct Model {
    #[sea_orm(primary_key, column_type = "Uuid")]
    pub id: Uuid,
    pub purchase_order_id: Option<Uuid>,
    pub product_id: Uuid,
    pub warehouse_id: Uuid,
    pub quantity: i32,
    pub received_at: DateTime<Utc>,
    pub notes: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

impl Model {
    pub fn new(product_id: Uuid, warehouse_id: Uuid, quantity: i32) -> Self {
        Self {
            id: Uuid::new_v4(),
            purchase_order_id: None,
            product_id,
            warehouse_id,
            quantity,
            received_at: Utc::now(),
            notes: None,
        }
    }
}
