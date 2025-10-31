use crate::circuit_breaker::CircuitBreaker;
use crate::message_queue::MessageQueue;
use crate::{
    commands::workorders::{
        assign_work_order_command::AssignWorkOrderCommand,
        // cancel_work_order_command::CancelWorkOrderCommand,
        // complete_work_order_command::CompleteWorkOrderCommand,
        // create_work_order_command::CreateWorkOrderCommand,
        // schedule_work_order_command::ScheduleWorkOrderCommand,
        // start_work_order_command::StartWorkOrderCommand,
        // unassign_work_order_command::UnassignWorkOrderCommand,
        // update_work_order_command::UpdateWorkOrderCommand,
    },
    commands::Command,
    db::DbPool,
    errors::ServiceError,
    events::EventSender,
    models::work_order::{
        self, ActiveModel as WorkOrderActiveModel, Model as WorkOrderModel, WorkOrderPriority,
        WorkOrderStatus,
    },
};
use chrono::{DateTime, NaiveDateTime, Utc};
use redis::Client as RedisClient;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, QuerySelect, Set,
};
use serde_json::{json, Value};
use slog::Logger;
use std::sync::Arc;
use tracing::instrument;
use uuid::Uuid;

/// Service for managing work orders
#[derive(Clone)]
pub struct WorkOrderService {
    db_pool: Arc<DbPool>,
    event_sender: Arc<EventSender>,
    redis_client: Arc<RedisClient>,
    message_queue: Arc<dyn MessageQueue>,
    circuit_breaker: Arc<CircuitBreaker>,
    logger: Logger,
}

#[derive(Debug, Clone)]
pub struct WorkOrderCreateData {
    pub title: String,
    pub description: Option<String>,
    pub status: Option<WorkOrderStatus>,
    pub priority: WorkOrderPriority,
    pub asset_id: Option<Uuid>,
    pub assigned_to: Option<Uuid>,
    pub due_date: Option<DateTime<Utc>>,
    pub bill_of_materials_number: Option<String>,
    pub quantity_produced: Option<i32>,
    pub parts_required: Option<Value>,
}

#[derive(Debug, Clone)]
pub struct WorkOrderUpdateData {
    pub title: Option<String>,
    pub description: Option<String>,
    pub status: Option<WorkOrderStatus>,
    pub priority: Option<WorkOrderPriority>,
    pub asset_id: Option<Uuid>,
    pub assigned_to: Option<Uuid>,
    pub due_date: Option<DateTime<Utc>>,
    pub bill_of_materials_number: Option<String>,
    pub quantity_produced: Option<i32>,
    pub parts_required: Option<Value>,
}

impl WorkOrderService {
    /// Creates a new work order service instance
    pub fn new(
        db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
        redis_client: Arc<RedisClient>,
        message_queue: Arc<dyn MessageQueue>,
        circuit_breaker: Arc<CircuitBreaker>,
        logger: Logger,
    ) -> Self {
        Self {
            db_pool,
            event_sender,
            redis_client,
            message_queue,
            circuit_breaker,
            logger,
        }
    }

    #[instrument(skip(self), err)]
    pub async fn create_work_order(
        &self,
        data: WorkOrderCreateData,
    ) -> Result<WorkOrderModel, ServiceError> {
        let db = self.db_pool.as_ref();
        let now = Utc::now();
        let parts_required = data.parts_required.unwrap_or_else(|| json!({}));

        let active = WorkOrderActiveModel {
            id: Set(Uuid::new_v4()),
            title: Set(data.title),
            description: Set(data.description),
            status: Set(data.status.unwrap_or(WorkOrderStatus::Pending)),
            priority: Set(data.priority),
            asset_id: Set(data.asset_id),
            assigned_to: Set(data.assigned_to),
            created_at: Set(now),
            updated_at: Set(now),
            due_date: Set(data.due_date),
            bill_of_materials_number: Set(data.bill_of_materials_number),
            quantity_produced: Set(data.quantity_produced),
            parts_required: Set(parts_required),
        };

        active.insert(db).await.map_err(ServiceError::db_error)
    }

    #[instrument(skip(self), err)]
    pub async fn update_work_order(
        &self,
        work_order_id: &Uuid,
        data: WorkOrderUpdateData,
    ) -> Result<WorkOrderModel, ServiceError> {
        let db = self.db_pool.as_ref();
        let existing = work_order::Entity::find_by_id(*work_order_id)
            .one(db)
            .await
            .map_err(ServiceError::db_error)?
            .ok_or_else(|| {
                ServiceError::NotFound(format!("Work order {} not found", work_order_id))
            })?;

        let mut active: WorkOrderActiveModel = existing.into();

        if let Some(title) = data.title {
            active.title = Set(title);
        }
        if let Some(description) = data.description {
            active.description = Set(Some(description));
        }
        if let Some(status) = data.status {
            active.status = Set(status);
        }
        if let Some(priority) = data.priority {
            active.priority = Set(priority);
        }
        if let Some(asset_id) = data.asset_id {
            active.asset_id = Set(Some(asset_id));
        }
        if let Some(assigned_to) = data.assigned_to {
            active.assigned_to = Set(Some(assigned_to));
        }
        if let Some(due_date) = data.due_date {
            active.due_date = Set(Some(due_date));
        }
        if let Some(bom) = data.bill_of_materials_number {
            active.bill_of_materials_number = Set(Some(bom));
        }
        if let Some(quantity) = data.quantity_produced {
            active.quantity_produced = Set(Some(quantity));
        }
        if let Some(parts) = data.parts_required {
            active.parts_required = Set(parts);
        }

        active.updated_at = Set(Utc::now());

        active.update(db).await.map_err(ServiceError::db_error)
    }

    #[instrument(skip(self), err)]
    pub async fn delete_work_order(&self, work_order_id: &Uuid) -> Result<(), ServiceError> {
        let db = self.db_pool.as_ref();
        let result = work_order::Entity::delete_by_id(*work_order_id)
            .exec(db)
            .await
            .map_err(ServiceError::db_error)?;

        if result.rows_affected == 0 {
            return Err(ServiceError::NotFound(format!(
                "Work order {} not found",
                work_order_id
            )));
        }

        Ok(())
    }

    #[instrument(skip(self), err)]
    pub async fn list_work_orders(
        &self,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<WorkOrderModel>, u64), ServiceError> {
        let db = self.db_pool.as_ref();
        let total = work_order::Entity::find()
            .count(db)
            .await
            .map_err(ServiceError::db_error)? as u64;

        let work_orders = work_order::Entity::find()
            .order_by_desc(work_order::Column::CreatedAt)
            .offset(page.saturating_sub(1) * page_size)
            .limit(page_size)
            .all(db)
            .await
            .map_err(ServiceError::db_error)?;

        Ok((work_orders, total))
    }

    /// Creates a new work order
    #[instrument(skip(self))]
    // /// Starts a work order
    // #[instrument(skip(self))]
    // // pub async fn start_work_order(
    //     &self,
    //     command: StartWorkOrderCommand,
    // ) -> Result<(), ServiceError> {
    //     command
    //         .execute(self.db_pool.clone(), self.event_sender.clone())
    //         .await?;
    //     Ok(())
    // }

    // /// Completes a work order
    // #[instrument(skip(self))]
    // // pub async fn complete_work_order(
    //     &self,
    //     command: CompleteWorkOrderCommand,
    // ) -> Result<(), ServiceError> {
    //     command
    //         .execute(self.db_pool.clone(), self.event_sender.clone())
    //         .await?;
    //     Ok(())
    // }

    /// Assigns a work order to a user
    #[instrument(skip(self))]
    pub async fn assign_work_order(
        &self,
        command: AssignWorkOrderCommand,
    ) -> Result<(), ServiceError> {
        command
            .execute(self.db_pool.clone(), self.event_sender.clone())
            .await?;
        Ok(())
    }

    // /// Unassigns a work order from a user
    // #[instrument(skip(self))]
    // // pub async fn unassign_work_order(
    //     &self,
    //     command: UnassignWorkOrderCommand,
    // ) -> Result<(), ServiceError> {
    //     command
    //         .execute(self.db_pool.clone(), self.event_sender.clone())
    //         .await?;
    //     Ok(())
    // }

    // /// Schedules a work order
    // #[instrument(skip(self))]
    // // pub async fn schedule_work_order(
    //     &self,
    //     command: ScheduleWorkOrderCommand,
    // ) -> Result<(), ServiceError> {
    //     command
    //         .execute(self.db_pool.clone(), self.event_sender.clone())
    //         .await?;
    //     Ok(())
    // }

    /// Gets a work order by ID
    #[instrument(skip(self))]
    pub async fn get_work_order(
        &self,
        work_order_id: &Uuid,
    ) -> Result<Option<work_order::Model>, ServiceError> {
        let db = &*self.db_pool;
        let work_order = work_order::Entity::find_by_id(*work_order_id)
            .one(db)
            .await
            .map_err(|e| ServiceError::db_error(e))?;

        Ok(work_order)
    }

    /// Gets work orders assigned to a user
    #[instrument(skip(self))]
    pub async fn get_work_orders_by_assignee(
        &self,
        user_id: &Uuid,
    ) -> Result<Vec<work_order::Model>, ServiceError> {
        let db = &*self.db_pool;
        let work_orders = work_order::Entity::find()
            .filter(work_order::Column::AssignedTo.eq(*user_id))
            .all(db)
            .await
            .map_err(|e| ServiceError::db_error(e))?;

        Ok(work_orders)
    }

    /// Gets work orders by status
    #[instrument(skip(self))]
    pub async fn get_work_orders_by_status(
        &self,
        status: &str,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<work_order::Model>, u64), ServiceError> {
        let db = &*self.db_pool;
        let filter = work_order::Column::Status.eq(status);

        let total = work_order::Entity::find()
            .filter(filter.clone())
            .count(db)
            .await
            .map_err(|e| ServiceError::db_error(e))? as u64;

        let work_orders = work_order::Entity::find()
            .filter(filter)
            .order_by_desc(work_order::Column::CreatedAt)
            .offset((page - 1) * page_size)
            .limit(page_size)
            .all(db)
            .await
            .map_err(|e| ServiceError::db_error(e))?;

        Ok((work_orders, total))
    }

    /// Gets work orders scheduled within a date range
    #[instrument(skip(self))]
    pub async fn get_work_orders_by_schedule(
        &self,
        start_date: NaiveDateTime,
        end_date: NaiveDateTime,
    ) -> Result<Vec<work_order::Model>, ServiceError> {
        let db = &*self.db_pool;
        let work_orders = work_order::Entity::find()
            .filter(work_order::Column::CreatedAt.gte(start_date))
            .filter(work_order::Column::DueDate.lte(end_date))
            .all(db)
            .await
            .map_err(|e| ServiceError::db_error(e))?;

        Ok(work_orders)
    }

    fn build_work_order_filters(
        &self,
        status: Option<String>,
        assignee_id: Option<Uuid>,
        start_date: Option<DateTime<Utc>>,
        end_date: Option<DateTime<Utc>>,
    ) -> sea_orm::Condition {
        let mut filter = sea_orm::Condition::all();

        if let Some(status) = status {
            filter = filter.add(work_order::Column::Status.eq(status));
        }

        if let Some(assignee_id) = assignee_id {
            filter = filter.add(work_order::Column::AssignedTo.eq(assignee_id));
        }

        if let Some(start_date) = start_date {
            filter = filter.add(work_order::Column::CreatedAt.gte(start_date));
        }

        if let Some(end_date) = end_date {
            filter = filter.add(work_order::Column::DueDate.lte(end_date));
        }

        filter
    }
}

#[cfg(all(test, feature = "mock-tests"))]
mod tests {
    // Unit tests temporarily disabled; manufacturing integration coverage exercises work orders.
}
