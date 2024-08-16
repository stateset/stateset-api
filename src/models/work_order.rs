use sea_orm::entity::prelude::*;
use serde::{Serialize, Deserialize};
use validator::{Validate, ValidationError};
use chrono::{NaiveDateTime, Utc};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize, Validate)]
#[sea_orm(table_name = "work_orders")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    
    #[validate(length(min = 1, max = 100, message = "Title must be between 1 and 100 characters"))]
    pub title: String,
    
    #[validate(length(max = 1000, message = "Description must not exceed 1000 characters"))]
    pub description: Option<String>,
    
    pub status: WorkOrderStatus,
    
    pub priority: WorkOrderPriority,
    
    #[validate(range(min = 1, message = "Assigned user ID must be positive"))]
    pub assigned_to: Option<i32>,
    
    pub due_date: Option<NaiveDateTime>,
    
    #[validate(range(min = 1, max = 10080, message = "Estimated duration must be between 1 and 10080 minutes (7 days)"))]
    pub estimated_duration: Option<i32>,
    
    #[validate(range(min = 1, message = "Actual duration must be positive"))]
    pub actual_duration: Option<i32>,
    
    pub created_at: DateTimeWithTimeZone,
    
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Validate)]
pub struct NewWorkOrder {
    #[validate(length(min = 1, max = 100, message = "Title must be between 1 and 100 characters"))]
    pub title: String,
    
    #[validate(length(max = 1000, message = "Description must not exceed 1000 characters"))]
    pub description: Option<String>,
    
    pub priority: WorkOrderPriority,
    
    pub due_date: Option<NaiveDateTime>,
    
    #[validate(range(min = 1, max = 10080, message = "Estimated duration must be between 1 and 10080 minutes (7 days)"))]
    pub estimated_duration: Option<i32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, EnumIter)]
#[sea_orm(rs_type = "String", db_type = "Text")]
pub enum WorkOrderStatus {
    #[sea_orm(string_value = "Pending")]
    Pending,
    
    #[sea_orm(string_value = "InProgress")]
    InProgress,
    
    #[sea_orm(string_value = "Completed")]
    Completed,
    
    #[sea_orm(string_value = "Cancelled")]
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, EnumIter)]
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