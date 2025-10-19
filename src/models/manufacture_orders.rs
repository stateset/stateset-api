use chrono::{DateTime, NaiveDate, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::{Validate, ValidationError};

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
    pub amount: rust_decimal::Decimal,
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

// Manufacture order line items moved to manufacture_order_line_item.rs

impl Model {
    pub fn new(
        number: String,
        priority: String,
        site: String,
        yield_location: String,
        expected_completion_date: NaiveDate,
        issued_on: NaiveDate,
        amount: rust_decimal::Decimal,
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
            amount,
        };
        manufacture_order
            .validate()
            .map_err(|_| ValidationError::new("Validation failed"))?;
        Ok(manufacture_order)
    }

    pub fn add_line_item(
        &self,
        line_item: super::manufacture_order_line_item::Model,
    ) -> Result<(), ValidationError> {
        // Here you would typically save the line item to the database
        // For this example, we'll just validate the line item
        line_item
            .validate()
            .map_err(|_| ValidationError::new("Validation failed"))?;
        Ok(())
    }
}

// ManufactureOrderLineItem implementation moved to manufacture_order_line_item.rs
