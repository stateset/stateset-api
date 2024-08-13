use diesel::prelude::*;
use serde::{Serialize, Deserialize};
use validator::Validate;
use crate::schema::work_orders;

#[derive(Debug, Serialize, Deserialize, Queryable, Insertable, AsChangeset, Validate)]
#[table_name = "work_orders"]
pub struct WorkOrder {
    pub id: i32,
    #[validate(length(min = 1, max = 100))]
    pub title: String,
    #[validate(length(min = 0, max = 1000))]
    pub description: Option<String>,
    pub status: WorkOrderStatus,
    pub priority: WorkOrderPriority,
    pub assigned_to: Option<i32>, // User ID
    pub due_date: Option<chrono::NaiveDateTime>,
    pub estimated_duration: Option<i32>, // In minutes
    pub actual_duration: Option<i32>, // In minutes
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
}

#[derive(Debug, Serialize, Deserialize, Insertable, Validate)]
#[table_name = "work_orders"]
pub struct NewWorkOrder {
    #[validate(length(min = 1, max = 100))]
    pub title: String,
    #[validate(length(min = 0, max = 1000))]
    pub description: Option<String>,
    pub priority: WorkOrderPriority,
    pub due_date: Option<chrono::NaiveDateTime>,
    pub estimated_duration: Option<i32>,
}

#[derive(Debug, Serialize, Deserialize, AsExpression, FromSqlRow)]
#[sql_type = "diesel::sql_types::Text"]
pub enum WorkOrderStatus {
    Pending,
    InProgress,
    Completed,
    Cancelled,
}

#[derive(Debug, Serialize, Deserialize, AsExpression, FromSqlRow)]
#[sql_type = "diesel::sql_types::Text"]
pub enum WorkOrderPriority {
    Low,
    Medium,
    High,
    Urgent,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WorkOrderSearchParams {
    pub title: Option<String>,
    pub status: Option<WorkOrderStatus>,
    pub priority: Option<WorkOrderPriority>,
    pub assigned_to: Option<i32>,
    pub due_date_from: Option<chrono::NaiveDateTime>,
    pub due_date_to: Option<chrono::NaiveDateTime>,
    pub limit: i64,
    pub offset: i64,
}