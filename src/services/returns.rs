use crate::circuit_breaker::CircuitBreaker;
use crate::commands::returns::{
    complete_return_command::CompleteReturnCommand,
    restock_returned_items_command::RestockReturnedItemsCommand,
};
use crate::message_queue::MessageQueue;
use crate::{
    // commands::returns::{
    // approve_return_command::ApproveReturnCommand, cancel_return_command::CancelReturnCommand,
    // complete_return_command::CompleteReturnCommand,
    // create_return_command::InitiateReturnCommand as CreateReturnCommand,
    // refund_return_command::RefundReturnCommand, reject_return_command::RejectReturnCommand,
    // restock_returned_items_command::RestockReturnedItemsCommand,
    // },
    commands::Command,
    db::DbPool,
    errors::ServiceError,
    events::{Event, EventSender},
    models::return_entity,
};
use anyhow::Result;
use redis::Client as RedisClient;
use sea_orm::PaginatorTrait;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
};
use slog::Logger;
use std::sync::Arc;
use tracing::{error, info, instrument};
use uuid::Uuid;

/// Service for managing returns
#[derive(Clone)]
pub struct ReturnService {
    db_pool: Arc<DbPool>,
    event_sender: Arc<EventSender>,
    redis_client: Arc<RedisClient>,
    message_queue: Arc<dyn MessageQueue>,
    circuit_breaker: Arc<CircuitBreaker>,
    logger: Logger,
}

impl ReturnService {
    /// Creates a new return service instance
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

    // /// Creates a new return
    // #[instrument(skip(self))]
    // // pub async fn create_return(
    //     &self,
    //     command: CreateReturnCommand,
    // ) -> Result<Uuid, ServiceError> {
    //     let result = command
    //         .execute(self.db_pool.clone(), self.event_sender.clone())
    //         .await?;
    // /// Approves a return
    // #[instrument(skip(self))]
    // // pub async fn approve_return(
    //     &self,
    //     command: ApproveReturnCommand,
    // ) -> Result<(), ServiceError> {
    //     command
    //         .execute(self.db_pool.clone(), self.event_sender.clone())
    //         .await?;
    // /// Rejects a return
    // #[instrument(skip(self))]
    // // pub async fn reject_return(
    //     &self,
    //     command: RejectReturnCommand,
    // ) -> Result<(), ServiceError> {
    //     command
    //         .execute(self.db_pool.clone(), self.event_sender.clone())
    //         .await?;
    // /// Cancels a return
    // #[instrument(skip(self))]
    // // pub async fn cancel_return(
    //     &self,
    //     command: CancelReturnCommand,
    // ) -> Result<(), ServiceError> {
    //     command
    //         .execute(self.db_pool.clone(), self.event_sender.clone())
    //         .await?;
    // /// Processes a refund for a return
    // #[instrument(skip(self))]
    // // pub async fn refund_return(
    //     &self,
    //     command: RefundReturnCommand,
    // ) -> Result<(), ServiceError> {
    //     command
    //         .execute(self.db_pool.clone(), self.event_sender.clone())
    //         .await?;
    //     Ok(())
    // }
    //     Ok(())
    // }
    //

    // /// Processes a refund for a return
    // #[instrument(skip(self))]
    // // pub async fn refund_return(
    //     &self,
    //     command: RefundReturnCommand,
    // ) -> Result<(), ServiceError> {
    //     command
    //         .execute(self.db_pool.clone(), self.event_sender.clone())
    //         .await?;
    //     Ok(())
    // }

    /// Completes a return process
    #[instrument(skip(self))]
    pub async fn complete_return(
        &self,
        command: CompleteReturnCommand,
    ) -> Result<(), ServiceError> {
        command
            .execute(self.db_pool.clone(), self.event_sender.clone())
            .await?;
        Ok(())
    }

    /// Restocks items from a return
    #[instrument(skip(self))]
    pub async fn restock_returned_items(
        &self,
        command: RestockReturnedItemsCommand,
    ) -> Result<(), ServiceError> {
        command
            .execute(self.db_pool.clone(), self.event_sender.clone())
            .await?;
        Ok(())
    }

    /// Gets a return by ID
    #[instrument(skip(self))]
    pub async fn get_return(
        &self,
        return_id: &Uuid,
    ) -> Result<Option<return_entity::Model>, ServiceError> {
        let db = &*self.db_pool;
        let ret = return_entity::Entity::find_by_id(*return_id)
            .one(db)
            .await
            .map_err(|e| {
                let msg = format!("Failed to get return: {}", e);
                error!(return_id = %return_id, error = %e, "Database error when fetching return");
                ServiceError::db_error(e)
            })?;

        Ok(ret)
    }

    /// Lists returns with pagination
    #[instrument(skip(self))]
    pub async fn list_returns(
        &self,
        page: u64,
        limit: u64,
    ) -> Result<(Vec<return_entity::Model>, u64), ServiceError> {
        use sea_orm::Paginator;

        let db = &*self.db_pool;

        // Create a paginator for the returns
        let paginator = return_entity::Entity::find()
            .order_by_desc(return_entity::Column::CreatedAt)
            .paginate(db, limit);

        // Get the total count of returns
        let total = paginator
            .num_items()
            .await
            .map_err(|e| ServiceError::db_error(e))?;

        let returns = paginator
            .fetch_page(page - 1)
            .await
            .map_err(|e| ServiceError::db_error(e))?;

        Ok((returns, total))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockall::mock;
    use mockall::predicate::*;
    use std::str::FromStr;
    use tokio::sync::broadcast;

    mock! {
        pub Database {}
        impl Clone for Database {
            fn clone(&self) -> Self;
        }
    }

    #[tokio::test]
    async fn test_create_return() {
        // Setup
        let (event_sender, _) = broadcast::channel(10);
        let event_sender = Arc::new(event_sender);
        let db_pool = Arc::new(MockDatabase::new());
        let redis_client = Arc::new(redis::Client::open("redis://localhost").unwrap());
        let message_queue = Arc::new(crate::message_queue::MockMessageQueue::new());
        let circuit_breaker = Arc::new(CircuitBreaker::new(
            5,
            std::time::Duration::from_secs(60),
            1,
        ));
        let logger = slog::Logger::root(slog::Discard, slog::o!());

        let service = ReturnService::new(
            db_pool,
            event_sender,
            redis_client,
            message_queue,
            circuit_breaker,
            logger,
        );

        // Test data
        let order_id = Uuid::from_str("00000000-0000-0000-0000-000000000001").unwrap();

        let command = CreateReturnCommand {
            order_id,
            reason: "Item damaged".to_string(),
            items: vec![],
        };

        // Execute
        let result = service.create_return(command).await;

        // Assert
        assert!(result.is_err()); // Will fail because we're using mock DB
    }
}
