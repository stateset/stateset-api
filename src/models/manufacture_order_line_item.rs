use chrono::NaiveDate;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::{Validate, ValidationError};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize, Validate)]
#[sea_orm(table_name = "manufacture_order_line_items")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: String,
    pub bom_name: String,
    pub bom_number: String,
    pub expected_date: NaiveDate,
    pub line_status: String,
    pub line_type: String,
    pub manufacture_order_number: String,
    pub output_type: String,
    pub part_name: String,
    pub part_number: String,
    pub priority: String,
    #[validate(range(min = 0, message = "Quantity must be non-negative"))]
    pub quantity: i32,
    pub site: String,
    pub work_order_number: String,
    pub unit_of_measure: String,
    pub yield_location: String,
    pub yield_item: String,
    pub expected_yield: f64,
    pub scrap_factor: Option<f64>,
    pub manufacture_order_id: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::manufacture_orders::Entity",
        from = "Column::ManufactureOrderId",
        to = "super::manufacture_orders::Column::Id"
    )]
    ManufactureOrder,
}

impl Related<super::manufacture_orders::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ManufactureOrder.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

/// Implementation for ManufactureOrderLineItem
impl Model {
    pub fn new(
        bom_name: String,
        bom_number: String,
        expected_date: NaiveDate,
        line_status: String,
        line_type: String,
        manufacture_order_number: String,
        output_type: String,
        part_name: String,
        part_number: String,
        priority: String,
        quantity: i32,
        site: String,
        work_order_number: String,
        yield_location: String,
        unit_of_measure: String,
        yield_item: String,
        expected_yield: f64,
        scrap_factor: Option<f64>,
        manufacture_order_id: String,
    ) -> Result<Self, ValidationError> {
        let item = Self {
            id: Uuid::new_v4().to_string(), // Generate a new UUID for the ID
            bom_name,
            bom_number,
            expected_date,
            line_status,
            line_type,
            manufacture_order_number,
            output_type,
            part_name,
            part_number,
            priority,
            quantity,
            site,
            work_order_number,
            unit_of_measure,
            yield_location,
            yield_item,
            expected_yield,
            scrap_factor,
            manufacture_order_id,
        };
        item.validate()
            .map_err(|_| ValidationError::new("Validation failed"))?;
        Ok(item)
    }

    pub fn update_status(&mut self, new_status: String) {
        self.line_status = new_status;
    }
}
