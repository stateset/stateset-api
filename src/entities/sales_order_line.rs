use rust_decimal::Decimal;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "sales_order_lines")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub line_id: i64,
    pub header_id: Option<i64>,
    pub line_number: Option<i32>,
    pub inventory_item_id: Option<i64>,
    #[sea_orm(column_type = "Decimal(Some((19, 4)))")]
    pub ordered_quantity: Option<Decimal>,
    #[sea_orm(column_type = "Decimal(Some((19, 4)))")]
    pub unit_selling_price: Option<Decimal>,
    pub line_status: Option<String>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
    pub location_id: Option<i32>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::sales_order_header::Entity",
        from = "Column::HeaderId",
        to = "super::sales_order_header::Column::HeaderId"
    )]
    SalesOrderHeader,
    #[sea_orm(
        belongs_to = "super::item_master::Entity",
        from = "Column::InventoryItemId",
        to = "super::item_master::Column::InventoryItemId"
    )]
    ItemMaster,
    #[sea_orm(has_many = "super::order_fulfillments::Entity")]
    OrderFulfillments,
    #[sea_orm(
        belongs_to = "super::inventory_location::Entity",
        from = "Column::LocationId",
        to = "super::inventory_location::Column::LocationId"
    )]
    InventoryLocation,
}

impl Related<super::sales_order_header::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::SalesOrderHeader.def()
    }
}

impl Related<super::item_master::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ItemMaster.def()
    }
}

impl Related<super::order_fulfillments::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::OrderFulfillments.def()
    }
}

impl Related<super::inventory_location::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::InventoryLocation.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}