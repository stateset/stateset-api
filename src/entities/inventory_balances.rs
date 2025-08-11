use chrono::{DateTime, Utc};
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
    pub quantity_on_hand: Decimal,
    pub quantity_allocated: Decimal,
    pub quantity_available: Decimal, // This is a generated column
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
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
        belongs_to = "super::inventory_locations::Entity",
        from = "Column::LocationId",
        to = "super::inventory_locations::Column::LocationId"
    )]
    InventoryLocation,
}

impl Related<super::item_master::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ItemMaster.def()
    }
}

impl Related<super::inventory_locations::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::InventoryLocation.def()
    }
}

impl ActiveModelBehavior for ActiveModel {} 