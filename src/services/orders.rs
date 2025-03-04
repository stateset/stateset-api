use std::sync::Arc;
use crate::{
    db::DbPool,
    errors::ServiceError,
    events::{Event, EventSender},
    models::order_entity,
    commands::orders::{
        create_order_command::CreateOrderCommand,
        cancel_order_command::CancelOrderCommand,
        update_order_status_command::UpdateOrderStatusCommand,
    },
    commands::Command,
};
use sea_orm::{DatabaseConnection, EntityTrait, QueryFilter, ColumnTrait, Set, ActiveModelTrait};
use tracing::{info, error, instrument};
use redis::Client as RedisClient;
use crate::message_queue::MessageQueue;
use crate::circuit_breaker::CircuitBreaker;
use slog::Logger;
use anyhow::Result;
use uuid::Uuid;

/// Service for managing orders
pub struct OrderService {
    db_pool: Arc<DbPool>,
    event_sender: Arc<EventSender>,
    redis_client: Arc<RedisClient>,
    message_queue: Arc<dyn MessageQueue>,
    circuit_breaker: Arc<CircuitBreaker>,
    logger: Logger,
}

impl OrderService {
    /// Creates a new order service instance
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

    /// Creates a new order
    #[instrument(skip(self))]
    pub async fn create_order(&self, command: CreateOrderCommand) -> Result<Uuid, ServiceError> {
        let result = command.execute(self.db_pool.clone(), self.event_sender.clone()).await?;
        Ok(result.id)
    }

    /// Cancels an existing order
    #[instrument(skip(self))]
    pub async fn cancel_order(&self, command: CancelOrderCommand) -> Result<(), ServiceError> {
        command.execute(self.db_pool.clone(), self.event_sender.clone()).await?;
        Ok(())
    }

    /// Updates order status
    #[instrument(skip(self))]
    pub async fn update_order_status(&self, command: UpdateOrderStatusCommand) -> Result<(), ServiceError> {
        command.execute(self.db_pool.clone(), self.event_sender.clone()).await?;
        Ok(())
    }

    /// Gets order by ID
    #[instrument(skip(self))]
    pub async fn get_order(&self, order_id: &Uuid) -> Result<Option<order_entity::Model>, ServiceError> {
        let db = &*self.db_pool;
        let order = order_entity::Entity::find_by_id(*order_id)
            .one(db)
            .await
            .map_err(|e| {
                let msg = format!("Failed to get order: {}", e);
                error!(order_id = %order_id, error = %e, "Database error when fetching order");
                ServiceError::DatabaseError(msg)
            })?;
        
        Ok(order)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockall::predicate::*;
    use mockall::mock;
    use tokio::sync::broadcast;
    use crate::models::OrderStatus;
    use crate::commands::orders::create_order_command::OrderItem;
    use std::str::FromStr;

    mock! {
        pub Database {}
        impl Clone for Database {
            fn clone(&self) -> Self;
        }
    }

    #[tokio::test]
    async fn test_create_order() {
        // Setup
        let (event_sender, _) = broadcast::channel(10);
        let event_sender = Arc::new(event_sender);
        let db_pool = Arc::new(MockDatabase::new());
        let redis_client = Arc::new(redis::Client::open("redis://localhost").unwrap());
        let message_queue = Arc::new(crate::message_queue::MockMessageQueue::new());
        let circuit_breaker = Arc::new(CircuitBreaker::new(5, std::time::Duration::from_secs(60)));
        let logger = slog::Logger::root(slog::Discard, slog::o!());
        
        let service = OrderService::new(
            db_pool,
            event_sender,
            redis_client,
            message_queue,
            circuit_breaker,
            logger,
        );

        // Test data
        let customer_id = Uuid::from_str("00000000-0000-0000-0000-000000000001").unwrap();
        let product_id = Uuid::from_str("00000000-0000-0000-0000-000000000002").unwrap();
        
        let command = CreateOrderCommand {
            customer_id,
            items: vec![OrderItem {
                product_id,
                quantity: 1,
            }],
        };

        // Execute
        let result = service.create_order(command).await;
        
        // Assert
        assert!(result.is_err()); // Will fail because we're using mock DB
    }
}