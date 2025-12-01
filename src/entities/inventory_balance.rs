use rust_decimal::Decimal;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "inventory_balances")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub inventory_balance_id: i64,
    pub inventory_item_id: i64,
    pub location_id: i32,
    #[sea_orm(column_type = "Decimal(Some((19, 4)))")]
    pub quantity_on_hand: Decimal,
    #[sea_orm(column_type = "Decimal(Some((19, 4)))")]
    pub quantity_allocated: Decimal,
    #[sea_orm(column_type = "Decimal(Some((19, 4)))", select_as = "expr")]
    pub quantity_available: Decimal,
    #[sea_orm(column_type = "Decimal(Some((19, 4)))")]
    pub reorder_point: Option<Decimal>,
    #[sea_orm(column_type = "Decimal(Some((19, 4)))")]
    pub safety_stock: Option<Decimal>,
    #[sea_orm(column_type = "Decimal(Some((19, 4)))")]
    pub reorder_quantity: Option<Decimal>,
    #[sea_orm(column_type = "Decimal(Some((19, 4)))")]
    pub max_stock_level: Option<Decimal>,
    pub lead_time_days: Option<i32>,
    pub version: i32,
    pub last_counted_at: Option<DateTimeWithTimeZone>,
    pub last_counted_by: Option<String>,
    pub deleted_at: Option<DateTimeWithTimeZone>,
    pub deleted_by: Option<String>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::item_master::Entity",
        from = "Column::InventoryItemId",
        to = "super::item_master::Column::InventoryItemId"
    )]
    ItemMaster,
    #[sea_orm(
        belongs_to = "super::inventory_location::Entity",
        from = "Column::LocationId",
        to = "super::inventory_location::Column::LocationId"
    )]
    InventoryLocation,
}

impl Related<super::item_master::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ItemMaster.def()
    }
}

impl Related<super::inventory_location::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::InventoryLocation.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
