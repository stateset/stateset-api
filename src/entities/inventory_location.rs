use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "inventory_locations")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub location_id: i32,
    pub location_name: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::inventory_balance::Entity")]
    InventoryBalances,
    #[sea_orm(has_many = "super::sales_order_header::Entity")]
    SalesOrderHeaders,
    #[sea_orm(has_many = "super::sales_order_line::Entity")]
    SalesOrderLines,
}

impl Related<super::inventory_balance::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::InventoryBalances.def()
    }
}

impl Related<super::sales_order_header::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::SalesOrderHeaders.def()
    }
}

impl Related<super::sales_order_line::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::SalesOrderLines.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}