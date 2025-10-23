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
    models::work_order,
};
use anyhow::Result;
use chrono::{DateTime, NaiveDateTime, Utc};
use redis::Client as RedisClient;
use sea_orm::{
    ColumnTrait, ConnectionTrait, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder, QuerySelect,
};
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
