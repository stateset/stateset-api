use rust_decimal::Decimal;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

// Reconcile Line Item Model
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize, Validate)]
#[sea_orm(table_name = "reconcile_line_items")]
pub struct Model {
    #[sea_orm(primary_key, column_type = "Uuid")]
    pub id: Uuid,
    pub line_type: String,
    pub status: String,
    pub part_number: String,
    pub part_name: String,
    pub vendor_part_number: String,
    pub vendor_part_name: String,
    pub order: String,
    pub quantity_billed: i32,
    #[sea_orm(column_type = "Decimal(Some((10, 2)))")]
    pub billed_unit_cost: Decimal,
    #[sea_orm(column_type = "Decimal(Some((10, 2)))")]
    pub billed_total_cost: Decimal,
    pub vendor: String,
    pub reconcile_number: i32,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::reconciles::Entity",
        from = "Column::ReconcileNumber",
        to = "super::reconciles::Column::Number"
    )]
    Reconcile,
}

impl Related<super::reconciles::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Reconcile.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

impl Model {
    pub fn new(
        line_type: String,
        status: String,
        part_number: String,
        part_name: String,
        vendor_part_number: String,
        vendor_part_name: String,
        order: String,
        quantity_billed: i32,
        billed_unit_cost: Decimal,
        vendor: String,
        reconcile_number: i32,
    ) -> Self {
        let billed_total_cost = Decimal::from(quantity_billed) * billed_unit_cost;
        Self {
            id: Uuid::new_v4(),
            line_type,
            status,
            part_number,
            part_name,
            vendor_part_number,
            vendor_part_name,
            order,
            quantity_billed,
            billed_unit_cost,
            billed_total_cost,
            vendor,
            reconcile_number,
        }
    }

    pub fn update_quantity(&mut self, new_quantity: i32) {
        self.quantity_billed = new_quantity;
        self.billed_total_cost = Decimal::from(self.quantity_billed) * self.billed_unit_cost;
    }

    pub fn update_unit_cost(&mut self, new_unit_cost: Decimal) {
        self.billed_unit_cost = new_unit_cost;
        self.billed_total_cost = Decimal::from(self.quantity_billed) * self.billed_unit_cost;
    }
}
