use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "purchase_order_items")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub purchase_order_id: Uuid,
    pub sku: String,
    pub product_name: String,
    pub description: Option<String>,
    pub quantity_ordered: i32,
    pub quantity_received: i32,
    pub unit_cost: Decimal,
    pub total_cost: Decimal,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "crate::models::purchase_order_entity::Entity",
        from = "Column::PurchaseOrderId",
        to = "crate::models::purchase_order_entity::Column::Id"
    )]
    PurchaseOrder,
}

impl Related<crate::models::purchase_order_entity::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::PurchaseOrder.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
