use rust_decimal::Decimal;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "bom_lines")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub bom_line_id: i64,
    pub bom_id: Option<i64>,
    pub component_item_id: Option<i64>,
    #[sea_orm(column_type = "Decimal(Some((19, 4)))")]
    pub quantity_per_assembly: Option<Decimal>,
    pub uom_code: Option<String>,
    pub operation_seq_num: Option<i32>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::bom_header::Entity",
        from = "Column::BomId",
        to = "super::bom_header::Column::BomId"
    )]
    BomHeader,
    #[sea_orm(
        belongs_to = "super::item_master::Entity",
        from = "Column::ComponentItemId",
        to = "super::item_master::Column::InventoryItemId"
    )]
    ComponentItem,
}

impl Related<super::bom_header::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::BomHeader.def()
    }
}

impl Related<super::item_master::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ComponentItem.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
