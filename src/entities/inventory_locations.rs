use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "inventory_locations")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = true)]
    pub location_id: i32,
    pub location_name: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::inventory_balances::Entity")]
    InventoryBalances,
}

impl Related<super::inventory_balances::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::InventoryBalances.def()
    }
}

impl ActiveModelBehavior for ActiveModel {} 