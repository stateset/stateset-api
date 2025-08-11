use std::sync::Arc;
use async_trait::async_trait;
use sea_orm::{
    DatabaseConnection, EntityTrait, QueryFilter, ColumnTrait, 
    QueryOrder, QuerySelect, Condition, IntoActiveModel, Set, ActiveModelTrait,
    TransactionTrait, DeleteResult,
};
use chrono::{NaiveDateTime, Utc};
use uuid::Uuid;

use crate::{
    db::DbPool,
    events::EventSender,
    errors::ServiceError,
    models::work_order::{
        Entity as WorkOrder, Model as WorkOrderModel, Column, 
        ActiveModel as WorkOrderActiveModel, WorkOrderStatus, WorkOrderPriority
    },
    commands::workorders::{
        create_work_order_command::CreateWorkOrderCommand,
        update_work_order_command::UpdateWorkOrderCommand,
        cancel_work_order_command::CancelWorkOrderCommand,
        start_work_order_command::StartWorkOrderCommand,
        complete_work_order_command::CompleteWorkOrderCommand,
        assign_work_order_command::AssignWorkOrderCommand,
        unassign_work_order_command::UnassignWorkOrderCommand,
        schedule_work_order_command::ScheduleWorkOrderCommand,
        add_note_to_work_order_command::AddNoteToWorkOrderCommand,
    },
};
use tracing::{info, error, instrument};

#[derive(Debug, Clone)]
pub struct WorkOrderService {
    db_pool: Arc<DbPool>,
    event_sender: Arc<EventSender>,
}

impl WorkOrderService {
    pub fn new(db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Self {
        Self { db_pool, event_sender }
    }

    #[instrument(skip(self), err)]
    pub async fn create_work_order(&self, command: CreateWorkOrderCommand) -> Result<Uuid, ServiceError> {
        let result = command.execute(self.db_pool.clone(), self.event_sender.clone()).await?;
        Ok(result.id)
    }

    #[instrument(skip(self), err)]
    pub async fn get_work_order(&self, id: &Uuid) -> Result<Option<WorkOrderModel>, ServiceError> {
        let db = self.db_pool.as_ref();
        
        let work_order = WorkOrder::find_by_id(*id)
            .one(db)
            .await
            .map_err(|e| {
                error!("Database error when fetching work order {}: {}", id, e);
                ServiceError::DatabaseError(format!("Failed to fetch work order: {}", e))
            })?;
        
        Ok(work_order)
    }

    #[instrument(skip(self), err)]
    pub async fn update_work_order(&self, command: UpdateWorkOrderCommand) -> Result<(), ServiceError> {
        command.execute(self.db_pool.clone(), self.event_sender.clone()).await?;
        Ok(())
    }

    #[instrument(skip(self), err)]
    pub async fn cancel_work_order(&self, command: CancelWorkOrderCommand) -> Result<(), ServiceError> {
        command.execute(self.db_pool.clone(), self.event_sender.clone()).await?;
        Ok(())
    }

    #[instrument(skip(self), err)]
    pub async fn start_work_order(&self, command: StartWorkOrderCommand) -> Result<(), ServiceError> {
        command.execute(self.db_pool.clone(), self.event_sender.clone()).await?;
        Ok(())
    }

    #[instrument(skip(self), err)]
    pub async fn complete_work_order(&self, command: CompleteWorkOrderCommand) -> Result<(), ServiceError> {
        command.execute(self.db_pool.clone(), self.event_sender.clone()).await?;
        Ok(())
    }

    #[instrument(skip(self), err)]
    pub async fn assign_work_order(&self, command: AssignWorkOrderCommand) -> Result<(), ServiceError> {
        command.execute(self.db_pool.clone(), self.event_sender.clone()).await?;
        Ok(())
    }

    #[instrument(skip(self), err)]
    pub async fn unassign_work_order(&self, command: UnassignWorkOrderCommand) -> Result<(), ServiceError> {
        command.execute(self.db_pool.clone(), self.event_sender.clone()).await?;
        Ok(())
    }

    #[instrument(skip(self), err)]
    pub async fn schedule_work_order(&self, command: ScheduleWorkOrderCommand) -> Result<(), ServiceError> {
        command.execute(self.db_pool.clone(), self.event_sender.clone()).await?;
        Ok(())
    }

    #[instrument(skip(self), err)]
    pub async fn add_note_to_work_order(&self, command: AddNoteToWorkOrderCommand) -> Result<i32, ServiceError> {
        let result = command.execute(self.db_pool.clone(), self.event_sender.clone()).await?;
        Ok(result.id)
    }

    #[instrument(skip(self), err)]
    pub async fn list_work_orders(&self, page: u64, page_size: u64) -> Result<(Vec<WorkOrderModel>, u64), ServiceError> {
        let db = self.db_pool.as_ref();
        let total = WorkOrder::find().count(db).await? as u64;
        let work_orders = WorkOrder::find()
            .order_by_desc(Column::CreatedAt)
            .offset((page - 1) * page_size)
            .limit(page_size)
            .all(db)
            .await?;
        Ok((work_orders, total))
    }

    #[instrument(skip(self), err)]
    pub async fn get_work_orders_by_assignee(
        &self,
        user_id: &Uuid,
    ) -> Result<Vec<WorkOrderModel>, ServiceError> {
        let db = self.db_pool.as_ref();
        
        // Assuming there's a created_by field that can be used for assignment
        // In a real implementation, you might have a specific assignee field
        let work_orders = WorkOrder::find()
            .filter(Column::CreatedBy.eq(user_id.to_string()))
            .order_by_desc(Column::CreatedAt)
            .all(db)
            .await
            .map_err(|e| {
                error!("Database error when fetching work orders by assignee {}: {}", user_id, e);
                ServiceError::DatabaseError(format!("Failed to fetch work orders by assignee: {}", e))
            })?;
        
        Ok(work_orders)
    }

    #[instrument(skip(self), err)]
    pub async fn get_work_orders_by_status(
        &self,
        status: &str,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<WorkOrderModel>, u64), ServiceError> {
        let db = self.db_pool.as_ref();
        // Parse the status string to our enum
        let parsed_status = match status.to_lowercase().as_str() {
            "pending" => WorkOrderStatus::Pending,
            "in progress" | "inprogress" => WorkOrderStatus::InProgress,
            "completed" => WorkOrderStatus::Completed,
            "cancelled" | "canceled" => WorkOrderStatus::Cancelled,
            _ => return Err(ServiceError::ValidationError(format!("Invalid status: {}", status))),
        };
        let total = WorkOrder::find()
            .filter(Column::Status.eq(parsed_status.clone()))
            .count(db)
            .await
            .map_err(|e| {
                error!("Database error when counting work orders by status {}: {}", status, e);
                ServiceError::DatabaseError(format!("Failed to count work orders by status: {}", e))
            })? as u64;
        let work_orders = WorkOrder::find()
            .filter(Column::Status.eq(parsed_status))
            .order_by_desc(Column::CreatedAt)
            .offset((page - 1) * page_size)
            .limit(page_size)
            .all(db)
            .await
            .map_err(|e| {
                error!("Database error when fetching work orders by status {}: {}", status, e);
                ServiceError::DatabaseError(format!("Failed to fetch work orders by status: {}", e))
            })?;
        Ok((work_orders, total))
    }

    #[instrument(skip(self), err)]
    pub async fn get_work_orders_by_schedule(
        &self,
        start_date: NaiveDateTime,
        end_date: NaiveDateTime,
    ) -> Result<Vec<WorkOrderModel>, ServiceError> {
        let db = self.db_pool.as_ref();
        
        // Convert NaiveDateTime to NaiveDate for comparison
        let start_date = start_date.date();
        let end_date = end_date.date();
        
        let work_orders = WorkOrder::find()
            .filter(
                Condition::all()
                    .add(Column::IssueDate.gte(start_date))
                    .add(Column::ExpectedCompletionDate.lte(end_date))
            )
            .order_by_asc(Column::IssueDate)
            .all(db)
            .await
            .map_err(|e| {
                error!("Database error when fetching work orders by schedule: {}", e);
                ServiceError::DatabaseError(format!("Failed to fetch work orders by schedule: {}", e))
            })?;
        
        Ok(work_orders)
    }

    #[instrument(skip(self), err)]
    pub async fn get_work_orders_count_by_status(&self) -> Result<Vec<(WorkOrderStatus, i64)>, ServiceError> {
        let db = self.db_pool.as_ref();
        
        // This is a simplified implementation
        // In a real-world scenario, you would use an aggregation query
        
        // Get all statuses
        let statuses = vec![
            WorkOrderStatus::Pending,
            WorkOrderStatus::InProgress,
            WorkOrderStatus::Completed,
            WorkOrderStatus::Cancelled,
        ];
        
        let mut results = vec![];
        
        for status in statuses {
            let count = WorkOrder::find()
                .filter(Column::Status.eq(status.clone()))
                .count(db)
                .await
                .map_err(|e| {
                    error!("Database error when counting work orders by status {}: {}", status, e);
                    ServiceError::DatabaseError(format!("Failed to count work orders by status: {}", e))
                })?;
            
            results.push((status, count));
        }
        
        Ok(results)
    }
}