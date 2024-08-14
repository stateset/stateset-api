use diesel::prelude::*;
use serde::{Serialize, Deserialize};
use validator::{Validate, ValidationError};
use chrono::{NaiveDateTime, Utc};
use crate::schema::work_orders;

#[derive(Debug, Clone, Serialize, Deserialize, Queryable, Insertable, AsChangeset, Validate)]
#[table_name = "work_orders"]
pub struct WorkOrder {
    pub id: i32,
    #[validate(length(min = 1, max = 100, message = "Title must be between 1 and 100 characters"))]
    pub title: String,
    #[validate(length(max = 1000, message = "Description must not exceed 1000 characters"))]
    pub description: Option<String>,
    pub status: WorkOrderStatus,
    pub priority: WorkOrderPriority,
    #[validate(range(min = 1, message = "Assigned user ID must be positive"))]
    pub assigned_to: Option<i32>, // User ID
    pub due_date: Option<NaiveDateTime>,
    #[validate(range(min = 1, max = 10080, message = "Estimated duration must be between 1 and 10080 minutes (7 days)"))]
    pub estimated_duration: Option<i32>, // In minutes
    #[validate(range(min = 1, message = "Actual duration must be positive"))]
    pub actual_duration: Option<i32>, // In minutes
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize, Insertable, Validate)]
#[table_name = "work_orders"]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, AsExpression, FromSqlRow)]
#[sql_type = "diesel::sql_types::Text"]
pub enum WorkOrderStatus {
    Pending,
    InProgress,
    Completed,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, AsExpression, FromSqlRow)]
#[sql_type = "diesel::sql_types::Text"]
pub enum WorkOrderPriority {
    Low,
    Medium,
    High,
    Urgent,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct WorkOrderSearchParams {
    #[validate(length(max = 100, message = "Search title must not exceed 100 characters"))]
    pub title: Option<String>,
    pub status: Option<WorkOrderStatus>,
    pub priority: Option<WorkOrderPriority>,
    #[validate(range(min = 1, message = "Assigned user ID must be positive"))]
    pub assigned_to: Option<i32>,
    pub due_date_from: Option<NaiveDateTime>,
    pub due_date_to: Option<NaiveDateTime>,
    #[validate(range(min = 1, max = 100, message = "Limit must be between 1 and 100"))]
    pub limit: i64,
    #[validate(range(min = 0, message = "Offset must be non-negative"))]
    pub offset: i64,
}

impl WorkOrder {
    pub fn new(new_work_order: NewWorkOrder) -> Result<Self, ValidationError> {
        let now = Utc::now().naive_utc();
        let work_order = Self {
            id: 0, // Assuming database will auto-increment this
            title: new_work_order.title,
            description: new_work_order.description,
            status: WorkOrderStatus::Pending,
            priority: new_work_order.priority,
            assigned_to: None,
            due_date: new_work_order.due_date,
            estimated_duration: new_work_order.estimated_duration,
            actual_duration: None,
            created_at: now,
            updated_at: now,
        };
        work_order.validate()?;
        Ok(work_order)
    }

    pub fn update_status(&mut self, new_status: WorkOrderStatus) -> Result<(), String> {
        if self.status == WorkOrderStatus::Completed || self.status == WorkOrderStatus::Cancelled {
            return Err("Cannot update status of a completed or cancelled work order".into());
        }
        self.status = new_status;
        self.updated_at = Utc::now().naive_utc();
        Ok(())
    }

    pub fn assign(&mut self, user_id: i32) -> Result<(), ValidationError> {
        self.assigned_to = Some(user_id);
        self.validate()?;
        self.updated_at = Utc::now().naive_utc();
        Ok(())
    }

    pub fn is_overdue(&self) -> bool {
        if let Some(due_date) = self.due_date {
            Utc::now().naive_utc() > due_date && self.status != WorkOrderStatus::Completed
        } else {
            false
        }
    }
}

impl WorkOrderStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            WorkOrderStatus::Pending => "Pending",
            WorkOrderStatus::InProgress => "In Progress",
            WorkOrderStatus::Completed => "Completed",
            WorkOrderStatus::Cancelled => "Cancelled",
        }
    }

    pub fn is_final(&self) -> bool {
        matches!(self, WorkOrderStatus::Completed | WorkOrderStatus::Cancelled)
    }
}

impl WorkOrderPriority {
    pub fn as_str(&self) -> &'static str {
        match self {
            WorkOrderPriority::Low => "Low",
            WorkOrderPriority::Medium => "Medium",
            WorkOrderPriority::High => "High",
            WorkOrderPriority::Urgent => "Urgent",
        }
    }

    pub fn as_int(&self) -> i32 {
        match self {
            WorkOrderPriority::Low => 1,
            WorkOrderPriority::Medium => 2,
            WorkOrderPriority::High => 3,
            WorkOrderPriority::Urgent => 4,
        }
    }
}

impl WorkOrderSearchParams {
    pub fn new(limit: i64, offset: i64) -> Result<Self, ValidationError> {
        let params = Self {
            title: None,
            status: None,
            priority: None,
            assigned_to: None,
            due_date_from: None,
            due_date_to: None,
            limit,
            offset,
        };
        params.validate()?;
        Ok(params)
    }
}