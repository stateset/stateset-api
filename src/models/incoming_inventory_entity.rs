use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(32))")]
pub enum IncomingInventoryStatus {
    #[sea_orm(string_value = "Pending")]
    Pending,
    #[sea_orm(string_value = "InTransit")]
    InTransit,
    #[sea_orm(string_value = "Received")]
    Received,
    #[sea_orm(string_value = "Cancelled")]
    Cancelled,
}

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "incoming_inventory")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub sku: String,
    pub quantity: i32,
    pub warehouse_id: Uuid,
    pub expected_date: Option<chrono::NaiveDate>,
    pub supplier_id: Option<Uuid>,
    pub purchase_order_id: Option<Uuid>,
    pub status: IncomingInventoryStatus,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "crate::models::facility_entity::Entity",
        from = "Column::WarehouseId",
        to = "crate::models::facility_entity::Column::Id"
    )]
    Warehouse,
}

impl Related<crate::models::facility_entity::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Warehouse.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
