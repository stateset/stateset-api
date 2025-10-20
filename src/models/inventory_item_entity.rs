use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "inventory_items")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub sku: String,
    #[sea_orm(column_type = "Uuid")]
    pub product_id: Uuid,
    pub product_name: String,
    pub description: Option<String>,
    pub warehouse_id: Uuid,
    pub quantity: i32,
    pub allocated_quantity: i32,
    pub available_quantity: i32,
    pub reserved_quantity: i32,
    pub unit_cost: Decimal,
    pub unit_price: Decimal,
    pub reorder_point: i32,
    pub max_stock_level: i32,
    pub location: Option<String>,
    pub location_id: Uuid,
    pub lot_number: Option<String>,
    pub expiration_date: Option<chrono::NaiveDate>,
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
    #[sea_orm(
        belongs_to = "crate::models::product_entity::Entity",
        from = "Column::ProductId",
        to = "crate::models::product_entity::Column::Id"
    )]
    Product,
    #[sea_orm(
        belongs_to = "crate::models::warehouse_location_entity::Entity",
        from = "Column::LocationId",
        to = "crate::models::warehouse_location_entity::Column::Id"
    )]
    WarehouseLocation,
}

impl Related<crate::models::facility_entity::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Warehouse.def()
    }
}

impl Related<crate::models::product_entity::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Product.def()
    }
}

impl Related<crate::models::warehouse_location_entity::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::WarehouseLocation.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
