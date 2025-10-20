use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "sales_order_headers")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub header_id: i64,
    pub order_number: String,
    pub order_type_id: Option<i64>,
    pub sold_to_org_id: Option<i64>,
    pub ordered_date: Option<Date>,
    pub status_code: Option<String>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
    pub location_id: Option<i32>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::sales_order_line::Entity")]
    SalesOrderLines,
    #[sea_orm(has_many = "super::order_fulfillments::Entity")]
    OrderFulfillments,
    #[sea_orm(
        belongs_to = "super::inventory_location::Entity",
        from = "Column::LocationId",
        to = "super::inventory_location::Column::LocationId"
    )]
    InventoryLocation,
}

impl Related<super::sales_order_line::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::SalesOrderLines.def()
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
