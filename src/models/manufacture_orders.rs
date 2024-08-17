use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use validator::Validate;
use chrono::{NaiveDate, DateTime, Utc};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize, Validate)]
#[sea_orm(table_name = "manufacture_orders")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: String,
    pub memo: Option<String>,
    pub number: String,
    pub priority: String,
    pub site: String,
    pub yield_location: String,
    pub created_on: DateTime<Utc>,
    pub expected_completion_date: NaiveDate,
    pub issued_on: NaiveDate,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::manufacture_order_line_item::Entity")]
    ManufactureOrderLineItems,
}

impl Related<super::manufacture_order_line_item::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ManufactureOrderLineItems.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize, Validate)]
#[sea_orm(table_name = "manufacture_order_line_items")]
pub struct ManufactureOrderLineItem {
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
    pub yield_location: String,
    pub manufacture_order_id: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum ManufactureOrderLineItemRelation {
    #[sea_orm(
        belongs_to = "super::manufacture_order::Entity",
        from = "Column::ManufactureOrderId",
        to = "super::manufacture_order::Column::Id"
    )]
    ManufactureOrder,
}

impl Related<super::manufacture_order::Entity> for ManufactureOrderLineItem {
    fn to() -> RelationDef {
        ManufactureOrderLineItemRelation::ManufactureOrder.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

impl Model {
    pub fn new(
        number: String,
        priority: String,
        site: String,
        yield_location: String,
        expected_completion_date: NaiveDate,
        issued_on: NaiveDate,
    ) -> Result<Self, ValidationError> {
        let manufacture_order = Self {
            id: Uuid::new_v4().to_string(), // Generate a new UUID for the ID
            memo: None,
            number,
            priority,
            site,
            yield_location,
            created_on: Utc::now(),
            expected_completion_date,
            issued_on,
        };
        manufacture_order.validate()?;
        Ok(manufacture_order)
    }

    pub fn add_line_item(&self, line_item: ManufactureOrderLineItem) -> Result<(), ValidationError> {
        // Here you would typically save the line item to the database
        // For this example, we'll just validate the line item
        line_item.validate()?;
        Ok(())
    }
}

impl ManufactureOrderLineItem {
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
            yield_location,
            manufacture_order_id,
        };
        item.validate()?;
        Ok(item)
    }

    pub fn update_status(&mut self, new_status: String) {
        self.line_status = new_status;
    }
}