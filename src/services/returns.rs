use std::sync::Arc;
use crate::{
    db::DbPool,
    errors::ServiceError,
    events::{Event, EventSender},
    models::return_entity,
    commands::returns::{
        create_return_command::InitiateReturnCommand as CreateReturnCommand,
        approve_return_command::ApproveReturnCommand,
        reject_return_command::RejectReturnCommand,
        cancel_return_command::CancelReturnCommand,
        refund_return_command::RefundReturnCommand,
        complete_return_command::CompleteReturnCommand,
        restock_returned_items_command::RestockReturnedItemsCommand,
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

/// Service for managing returns
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

    /// Creates a new return
    #[instrument(skip(self))]
    pub async fn create_return(&self, command: CreateReturnCommand) -> Result<Uuid, ServiceError> {
        let result = command.execute(self.db_pool.clone(), self.event_sender.clone()).await?;
        Ok(result.id)
    }

    /// Approves a return
    #[instrument(skip(self))]
    pub async fn approve_return(&self, command: ApproveReturnCommand) -> Result<(), ServiceError> {
        command.execute(self.db_pool.clone(), self.event_sender.clone()).await?;
        Ok(())
    }

    /// Rejects a return
    #[instrument(skip(self))]
    pub async fn reject_return(&self, command: RejectReturnCommand) -> Result<(), ServiceError> {
        command.execute(self.db_pool.clone(), self.event_sender.clone()).await?;
        Ok(())
    }

    /// Cancels a return
    #[instrument(skip(self))]
    pub async fn cancel_return(&self, command: CancelReturnCommand) -> Result<(), ServiceError> {
        command.execute(self.db_pool.clone(), self.event_sender.clone()).await?;
        Ok(())
    }

    /// Processes a refund for a return
    #[instrument(skip(self))]
    pub async fn refund_return(&self, command: RefundReturnCommand) -> Result<(), ServiceError> {
        command.execute(self.db_pool.clone(), self.event_sender.clone()).await?;
        Ok(())
    }

    /// Completes a return process
    #[instrument(skip(self))]
    pub async fn complete_return(&self, command: CompleteReturnCommand) -> Result<(), ServiceError> {
        command.execute(self.db_pool.clone(), self.event_sender.clone()).await?;
        Ok(())
    }

    /// Restocks items from a return
    #[instrument(skip(self))]
    pub async fn restock_returned_items(&self, command: RestockReturnedItemsCommand) -> Result<(), ServiceError> {
        command.execute(self.db_pool.clone(), self.event_sender.clone()).await?;
        Ok(())
    }

    /// Gets a return by ID
    #[instrument(skip(self))]
    pub async fn get_return(&self, return_id: &Uuid) -> Result<Option<return_entity::Model>, ServiceError> {
        let db = &*self.db_pool;
        let ret = return_entity::Entity::find_by_id(*return_id)
            .one(db)
            .await
            .map_err(|e| {
                let msg = format!("Failed to get return: {}", e);
                error!(return_id = %return_id, error = %e, "Database error when fetching return");
                ServiceError::DatabaseError(msg)
            })?;
        
        Ok(ret)
    }
    
    /// Lists returns with pagination
    #[instrument(skip(self))]
    pub async fn list_returns(&self, page: u64, limit: u64) -> Result<(Vec<return_entity::Model>, u64), ServiceError> {
        use sea_orm::{Paginator, PaginatorTrait};
        
        let db = &*self.db_pool;
        
        // Create a paginator for the returns
        let paginator = return_entity::Entity::find()
            .order_by_desc(return_entity::Column::CreatedAt)
            .paginate(db, limit);
            
        // Get the total count of returns
        let total = paginator.num_items().await
            .map_err(|e| {
                let msg = format!("Failed to count returns: {}", e);
                error!(error = %e, "Database error when counting returns");
                ServiceError::DatabaseError(msg)
            })?;
            
        // Get the requested page of returns
        let returns = paginator.fetch_page(page - 1).await
            .map_err(|e| {
                let msg = format!("Failed to fetch returns: {}", e);
                error!(page = %page, limit = %limit, error = %e, "Database error when fetching returns");
                ServiceError::DatabaseError(msg)
            })?;
            
        Ok((returns, total))
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
    async fn test_create_return() {
        // Setup
        let (event_sender, _) = broadcast::channel(10);
        let event_sender = Arc::new(event_sender);
        let db_pool = Arc::new(MockDatabase::new());
        let redis_client = Arc::new(redis::Client::open("redis://localhost").unwrap());
        let message_queue = Arc::new(crate::message_queue::MockMessageQueue::new());
        let circuit_breaker = Arc::new(CircuitBreaker::new(5, std::time::Duration::from_secs(60), 1));
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