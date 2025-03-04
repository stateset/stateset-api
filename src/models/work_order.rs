use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use validator::Validate;
use chrono::{DateTime, NaiveDate, Utc};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize, Validate)]
#[sea_orm(table_name = "work_orders")]
pub struct Model {
    #[sea_orm(primary_key, column_type = "Uuid")]
    pub id: Uuid,
    pub number: i32,
    pub site: String,
    pub work_order_type: String,
    pub location: String,
    pub part: String,
    pub order_number: String,
    pub manufacture_order: String,
    pub status: WorkOrderStatus,
    pub created_by: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub issue_date: NaiveDate,
    pub expected_completion_date: NaiveDate,
    pub priority: WorkOrderPriority,
    pub memo: Option<String>,
    pub bill_of_materials_number: i32,
    #[validate(range(min = 0.0, message = "Actual labor hours must be non-negative"))]
    pub actual_labor_hours: f64,
    #[validate(range(min = 0.0, message = "Standard labor hours must be non-negative"))]
    pub standard_labor_hours: f64,
    #[sea_orm(column_type = "Uuid")]
    pub capacity_utilization_id: Uuid,
    #[sea_orm(column_type = "Uuid")]
    pub bill_of_materials_id: Uuid,
    #[sea_orm(column_type = "Uuid")]
    pub cogs_data_id: Uuid,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::work_order_line_item::Entity")]
    WorkOrderLineItems,
}

impl Related<super::work_order_line_item::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::WorkOrderLineItems.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "Text")]
pub enum WorkOrderStatus {
    #[sea_orm(string_value = "Pending")]
    Pending,
    #[sea_orm(string_value = "In Progress")]
    InProgress,
    #[sea_orm(string_value = "Completed")]
    Completed,
    #[sea_orm(string_value = "Cancelled")]
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "Text")]
pub enum WorkOrderPriority {
    #[sea_orm(string_value = "Low")]
    Low,
    #[sea_orm(string_value = "Medium")]
    Medium,
    #[sea_orm(string_value = "High")]
    High,
    #[sea_orm(string_value = "Urgent")]
    Urgent,
}


impl Model {
    pub fn new(
        number: i32,
        site: String,
        work_order_type: String,
        location: String,
        part: String,
        order_number: String,
        manufacture_order: String,
        created_by: String,
        issue_date: NaiveDate,
        expected_completion_date: NaiveDate,
        priority: WorkOrderPriority,
        bill_of_materials_number: i32,
        capacity_utilization_id: Uuid,
        bill_of_materials_id: Uuid,
        cogs_data_id: Uuid,
    ) -> Result<Self, ValidationError> {
        let now = Utc::now();
        let work_order = Self {
            id: Uuid::new_v4(),
            number,
            site,
            work_order_type,
            location,
            part,
            order_number,
            manufacture_order,
            status: WorkOrderStatus::Pending,
            created_by,
            created_at: now,
            updated_at: now,
            issue_date,
            expected_completion_date,
            priority,
            memo: None,
            bill_of_materials_number,
            actual_labor_hours: 0.0,
            standard_labor_hours: 0.0,
            capacity_utilization_id,
            bill_of_materials_id,
            cogs_data_id,
        };
        work_order.validate()?;
        Ok(work_order)
    }

    pub fn update_status(&mut self, new_status: WorkOrderStatus) -> Result<(), String> {
        if self.status == WorkOrderStatus::Completed || self.status == WorkOrderStatus::Cancelled {
            return Err("Cannot update status of a completed or cancelled work order".into());
        }
        self.status = new_status;
        self.updated_at = Utc::now();
        Ok(())
    }

    pub fn add_line_item(&self, line_item: super::work_order_line_item::Model) -> Result<(), ValidationError> {
        // Here you would typically save the line item to the database
        // For this example, we'll just validate the line item
        line_item.validate()?;
        Ok(())
    }
}