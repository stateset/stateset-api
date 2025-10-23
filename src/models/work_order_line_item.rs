use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize, Validate)]
#[sea_orm(table_name = "work_order_line_items")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub line_status: String,
    pub line_type: String,
    pub part_name: String,
    pub part_number: String,
    #[validate(range(min = 0.0, message = "Total quantity must be non-negative"))]
    pub total_quantity: f64,
    #[validate(range(min = 0.0, message = "Picked quantity must be non-negative"))]
    pub picked_quantity: f64,
    #[validate(range(min = 0.0, message = "Issued quantity must be non-negative"))]
    pub issued_quantity: f64,
    #[validate(range(min = 0.0, message = "Yielded quantity must be non-negative"))]
    pub yielded_quantity: f64,
    #[validate(range(min = 0.0, message = "Scrapped quantity must be non-negative"))]
    pub scrapped_quantity: f64,
    pub unit_of_measure: String,
    #[sea_orm(column_type = "Uuid")]
    pub work_order_id: Uuid,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::work_order::Entity",
        from = "Column::WorkOrderId",
        to = "super::work_order::Column::Id"
    )]
    WorkOrder,
}

impl Related<super::work_order::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::WorkOrder.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
