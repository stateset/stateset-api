use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "bom_headers")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub bom_id: i64,
    pub bom_name: String,
    pub item_id: Option<i64>,
    pub organization_id: i64,
    pub revision: Option<String>,
    pub status_code: Option<String>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::item_master::Entity",
        from = "Column::ItemId",
        to = "super::item_master::Column::InventoryItemId"
    )]
    ItemMaster,
    #[sea_orm(has_many = "super::bom_line::Entity")]
    BomLines,
}

impl Related<super::item_master::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ItemMaster.def()
    }
}

impl Related<super::bom_line::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::BomLines.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
