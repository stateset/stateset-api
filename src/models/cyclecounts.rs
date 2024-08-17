use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use validator::Validate;
use chrono::NaiveDate;
use uuid::Uuid;

// Cycle Count Model
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize, Validate)]
#[sea_orm(table_name = "cycle_counts")]
pub struct Model {
    #[sea_orm(primary_key, column_type = "Uuid")]
    pub id: Uuid,
    pub number: Option<i32>,
    pub site: Option<String>,
    #[sea_orm(column_name = "type")]
    pub cycle_type: Option<String>,
    pub method: Option<String>,
    pub status: Option<String>,
    pub scheduled_start_date: Option<NaiveDate>,
    pub scheduled_end_date: Option<NaiveDate>,
    pub completed_date: Option<NaiveDate>,
    pub assigned_user: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::cycle_count_line_item::Entity")]
    CycleCountLineItems,
}

impl Related<super::cycle_count_line_item::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::CycleCountLineItems.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

// Cycle Count Line Item Model
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize, Validate)]
#[sea_orm(table_name = "cycle_count_line_items")]
pub struct CycleCountLineItem {
    #[sea_orm(primary_key, column_type = "Uuid")]
    pub id: Uuid,
    pub cycle_count_number: Option<i32>,
    pub status: Option<String>,
    pub part: Option<String>,
    pub standard_tracking: Option<String>,
    pub serialized_tracking: Option<String>,
    pub quantity_expected: Option<i32>,
    pub quantity_counted: Option<i32>,
    pub variance_quantity: Option<i32>,
    pub variance_cost: Option<i32>,
    pub explanation: Option<String>,
    #[sea_orm(column_type = "Uuid")]
    pub cycle_count_id: Uuid,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum CycleCountLineItemRelation {
    #[sea_orm(
        belongs_to = "super::cycle_count::Entity",
        from = "Column::CycleCountId",
        to = "super::cycle_count::Column::Id"
    )]
    CycleCount,
}

impl Related<super::cycle_count::Entity> for CycleCountLineItem {
    fn to() -> RelationDef {
        CycleCountLineItemRelation::CycleCount.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

impl Model {
    pub fn new(site: String, cycle_type: String, method: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            number: None,
            site: Some(site),
            cycle_type: Some(cycle_type),
            method: Some(method),
            status: Some("Scheduled".to_string()),
            scheduled_start_date: None,
            scheduled_end_date: None,
            completed_date: None,
            assigned_user: None,
        }
    }

    pub fn set_dates(&mut self, start: NaiveDate, end: NaiveDate) {
        self.scheduled_start_date = Some(start);
        self.scheduled_end_date = Some(end);
    }

    pub fn assign_user(&mut self, user: String) {
        self.assigned_user = Some(user);
    }

    pub fn complete(&mut self, completion_date: NaiveDate) {
        self.status = Some("Completed".to_string());
        self.completed_date = Some(completion_date);
    }

    pub fn add_line_item(&self, line_item: CycleCountLineItem) -> Result<(), ValidationError> {
        // Here you would typically save the line item to the database
        // For this example, we'll just validate the line item
        line_item.validate()?;
        Ok(())
    }
}

impl CycleCountLineItem {
    pub fn new(cycle_count_id: Uuid, part: String, quantity_expected: i32) -> Self {
        Self {
            id: Uuid::new_v4(),
            cycle_count_number: None,
            status: Some("Pending".to_string()),
            part: Some(part),
            standard_tracking: None,
            serialized_tracking: None,
            quantity_expected: Some(quantity_expected),
            quantity_counted: None,
            variance_quantity: None,
            variance_cost: None,
            explanation: None,
            cycle_count_id,
        }
    }

    pub fn record_count(&mut self, quantity_counted: i32) {
        self.quantity_counted = Some(quantity_counted);
        if let Some(expected) = self.quantity_expected {
            self.variance_quantity = Some(quantity_counted - expected);
        }
        self.status = Some("Counted".to_string());
    }

    pub fn set_variance_cost(&mut self, cost: i32) {
        self.variance_cost = Some(cost);
    }

    pub fn add_explanation(&mut self, explanation: String) {
        self.explanation = Some(explanation);
    }
}