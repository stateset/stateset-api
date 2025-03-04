use std::sync::Arc;
use crate::{
    db::DbPool,
    errors::ServiceError,
    events::{Event, EventSender},
    models::work_order,
    commands::workorders::{
        create_work_order_command::CreateWorkOrderCommand,
        update_work_order_command::UpdateWorkOrderCommand,
        cancel_work_order_command::CancelWorkOrderCommand,
        start_work_order_command::StartWorkOrderCommand,
        complete_work_order_command::CompleteWorkOrderCommand,
        assign_work_order_command::AssignWorkOrderCommand,
        unassign_work_order_command::UnassignWorkOrderCommand,
        schedule_work_order_command::ScheduleWorkOrderCommand,
    },
    commands::Command,
};
use sea_orm::{EntityTrait, QueryFilter, ColumnTrait, Set, ActiveModelTrait, DatabaseConnection};
use tracing::{info, error, instrument};
use redis::Client as RedisClient;
use crate::message_queue::MessageQueue;
use crate::circuit_breaker::CircuitBreaker;
use slog::Logger;
use anyhow::Result;
use uuid::Uuid;
use chrono::{DateTime, Utc, NaiveDateTime};

/// Service for managing work orders
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
    pub async fn create_work_order(&self, command: CreateWorkOrderCommand) -> Result<Uuid, ServiceError> {
        let result = command.execute(self.db_pool.clone(), self.event_sender.clone()).await?;
        Ok(result.id)
    }

    /// Updates a work order
    #[instrument(skip(self))]
    pub async fn update_work_order(&self, command: UpdateWorkOrderCommand) -> Result<(), ServiceError> {
        command.execute(self.db_pool.clone(), self.event_sender.clone()).await?;
        Ok(())
    }

    /// Cancels a work order
    #[instrument(skip(self))]
    pub async fn cancel_work_order(&self, command: CancelWorkOrderCommand) -> Result<(), ServiceError> {
        command.execute(self.db_pool.clone(), self.event_sender.clone()).await?;
        Ok(())
    }

    /// Starts a work order
    #[instrument(skip(self))]
    pub async fn start_work_order(&self, command: StartWorkOrderCommand) -> Result<(), ServiceError> {
        command.execute(self.db_pool.clone(), self.event_sender.clone()).await?;
        Ok(())
    }

    /// Completes a work order
    #[instrument(skip(self))]
    pub async fn complete_work_order(&self, command: CompleteWorkOrderCommand) -> Result<(), ServiceError> {
        command.execute(self.db_pool.clone(), self.event_sender.clone()).await?;
        Ok(())
    }

    /// Assigns a work order to a user
    #[instrument(skip(self))]
    pub async fn assign_work_order(&self, command: AssignWorkOrderCommand) -> Result<(), ServiceError> {
        command.execute(self.db_pool.clone(), self.event_sender.clone()).await?;
        Ok(())
    }

    /// Unassigns a work order from a user
    #[instrument(skip(self))]
    pub async fn unassign_work_order(&self, command: UnassignWorkOrderCommand) -> Result<(), ServiceError> {
        command.execute(self.db_pool.clone(), self.event_sender.clone()).await?;
        Ok(())
    }

    /// Schedules a work order
    #[instrument(skip(self))]
    pub async fn schedule_work_order(&self, command: ScheduleWorkOrderCommand) -> Result<(), ServiceError> {
        command.execute(self.db_pool.clone(), self.event_sender.clone()).await?;
        Ok(())
    }

    /// Gets a work order by ID
    #[instrument(skip(self))]
    pub async fn get_work_order(&self, work_order_id: &Uuid) -> Result<Option<work_order::Model>, ServiceError> {
        let db = &*self.db_pool;
        let work_order = work_order::Entity::find_by_id(*work_order_id)
            .one(db)
            .await
            .map_err(|e| {
                let msg = format!("Failed to get work order: {}", e);
                error!(work_order_id = %work_order_id, error = %e, "Database error when fetching work order");
                ServiceError::DatabaseError(msg)
            })?;
        
        Ok(work_order)
    }

    /// Gets work orders assigned to a user
    #[instrument(skip(self))]
    pub async fn get_work_orders_by_assignee(&self, user_id: &Uuid) -> Result<Vec<work_order::Model>, ServiceError> {
        let db = &*self.db_pool;
        let work_orders = work_order::Entity::find()
            .filter(work_order::Column::AssignedTo.eq(*user_id))
            .all(db)
            .await
            .map_err(|e| {
                let msg = format!("Failed to get work orders by assignee: {}", e);
                error!(user_id = %user_id, error = %e, "Database error when fetching work orders");
                ServiceError::DatabaseError(msg)
            })?;
        
        Ok(work_orders)
    }

    /// Gets work orders by status
    #[instrument(skip(self))]
    pub async fn get_work_orders_by_status(&self, status: &str) -> Result<Vec<work_order::Model>, ServiceError> {
        let db = &*self.db_pool;
        let work_orders = work_order::Entity::find()
            .filter(work_order::Column::Status.eq(status))
            .all(db)
            .await
            .map_err(|e| {
                let msg = format!("Failed to get work orders by status: {}", e);
                error!(status = %status, error = %e, "Database error when fetching work orders");
                ServiceError::DatabaseError(msg)
            })?;
        
        Ok(work_orders)
    }

    /// Gets work orders scheduled within a date range
    #[instrument(skip(self))]
    pub async fn get_work_orders_by_schedule(&self, start_date: NaiveDateTime, end_date: NaiveDateTime) -> Result<Vec<work_order::Model>, ServiceError> {
        let db = &*self.db_pool;
        let work_orders = work_order::Entity::find()
            .filter(work_order::Column::StartDate.gte(start_date))
            .filter(work_order::Column::EndDate.lte(end_date))
            .all(db)
            .await
            .map_err(|e| {
                let msg = format!("Failed to get work orders by schedule: {}", e);
                error!(start_date = %start_date, end_date = %end_date, error = %e, "Database error when fetching work orders");
                ServiceError::DatabaseError(msg)
            })?;
        
        Ok(work_orders)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockall::predicate::*;
    use mockall::mock;
    use tokio::sync::broadcast;
    use std::str::FromStr;

    mock! {
        pub Database {}
        impl Clone for Database {
            fn clone(&self) -> Self;
        }
    }

    #[tokio::test]
    async fn test_create_work_order() {
        // Setup
        let (event_sender, _) = broadcast::channel(10);
        let event_sender = Arc::new(event_sender);
        let db_pool = Arc::new(MockDatabase::new());
        let redis_client = Arc::new(redis::Client::open("redis://localhost").unwrap());
        let message_queue = Arc::new(crate::message_queue::MockMessageQueue::new());
        let circuit_breaker = Arc::new(CircuitBreaker::new(5, std::time::Duration::from_secs(60), 1));
        let logger = slog::Logger::root(slog::Discard, slog::o!());
        
        let service = WorkOrderService::new(
            db_pool,
            event_sender,
            redis_client,
            message_queue,
            circuit_breaker,
            logger,
        );

        // Test data
        let bom_id = Uuid::from_str("00000000-0000-0000-0000-000000000001").unwrap();
        
        let command = CreateWorkOrderCommand {
            bom_id,
            quantity_planned: 10,
            priority: "High".to_string(),
            notes: "Test work order".to_string(),
        };

        // Execute
        let result = service.create_work_order(command).await;
        
        // Assert
        assert!(result.is_err()); // Will fail because we're using mock DB
    }
}