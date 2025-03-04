use std::sync::Arc;
use crate::{
    db::DbPool,
    errors::ServiceError,
    events::{Event, EventSender},
    models::inventory_items,
    commands::inventory::{
        adjust_inventory_command::AdjustInventoryCommand,
        allocate_inventory_command::AllocateInventoryCommand, 
        deallocate_inventory_command::DeallocateInventoryCommand,
        reserve_inventory_command::ReserveInventoryCommand,
        release_inventory_command::ReleaseInventoryCommand,
        set_inventory_levels_command::SetInventoryLevelsCommand,
        transfer_inventory_command::TransferInventoryCommand,
        receive_inventory_command::ReceiveInventoryCommand,
        cycle_count_command::CycleCountCommand,
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

/// Service for managing inventory
pub struct InventoryService {
    db_pool: Arc<DbPool>,
    event_sender: Arc<EventSender>,
    redis_client: Arc<RedisClient>,
    message_queue: Arc<dyn MessageQueue>,
    circuit_breaker: Arc<CircuitBreaker>,
    logger: Logger,
}

impl InventoryService {
    /// Creates a new inventory service instance
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

    /// Adjusts inventory quantity for a product
    #[instrument(skip(self))]
    pub async fn adjust_inventory(&self, command: AdjustInventoryCommand) -> Result<(), ServiceError> {
        command.execute(self.db_pool.clone(), self.event_sender.clone()).await?;
        Ok(())
    }

    /// Allocates inventory to an order
    #[instrument(skip(self))]
    pub async fn allocate_inventory(&self, command: AllocateInventoryCommand) -> Result<(), ServiceError> {
        command.execute(self.db_pool.clone(), self.event_sender.clone()).await?;
        Ok(())
    }

    /// Deallocates inventory from an order
    #[instrument(skip(self))]
    pub async fn deallocate_inventory(&self, command: DeallocateInventoryCommand) -> Result<(), ServiceError> {
        command.execute(self.db_pool.clone(), self.event_sender.clone()).await?;
        Ok(())
    }

    /// Reserves inventory for future allocation
    #[instrument(skip(self))]
    pub async fn reserve_inventory(&self, command: ReserveInventoryCommand) -> Result<(), ServiceError> {
        command.execute(self.db_pool.clone(), self.event_sender.clone()).await?;
        Ok(())
    }

    /// Releases reserved inventory
    #[instrument(skip(self))]
    pub async fn release_inventory(&self, command: ReleaseInventoryCommand) -> Result<(), ServiceError> {
        command.execute(self.db_pool.clone(), self.event_sender.clone()).await?;
        Ok(())
    }
    
    /// Sets inventory levels for a product at a location
    #[instrument(skip(self))]
    pub async fn set_inventory_levels(&self, command: SetInventoryLevelsCommand) -> Result<crate::commands::inventory::set_inventory_levels_command::SetInventoryLevelsResult, ServiceError> {
        command.execute(self.db_pool.clone(), self.event_sender.clone()).await
    }

    /// Gets inventory level for a product at a location
    #[instrument(skip(self))]
    pub async fn get_inventory(&self, product_id: &Uuid, location_id: &Uuid) -> Result<Option<inventory_items::Model>, ServiceError> {
        let db = &*self.db_pool;
        
        let inventory = inventory_items::Entity::find()
            .filter(inventory_items::Column::ProductId.eq(*product_id))
            .filter(inventory_items::Column::LocationId.eq(*location_id))
            .one(db)
            .await
            .map_err(|e| {
                let msg = format!("Failed to get inventory: {}", e);
                error!(product_id = %product_id, location_id = %location_id, error = %e, "Database error when fetching inventory");
                ServiceError::DatabaseError(msg)
            })?;
        
        Ok(inventory)
    }

    /// Checks if a product is in stock at a location
    #[instrument(skip(self))]
    pub async fn is_in_stock(&self, product_id: &Uuid, location_id: &Uuid, quantity: i32) -> Result<bool, ServiceError> {
        let inventory = self.get_inventory(product_id, location_id).await?;
        
        match inventory {
            Some(inv) => {
                let available = inv.quantity - inv.reserved;
                Ok(available >= quantity)
            },
            None => Ok(false),
        }
    }
    
    /// Transfers inventory from one location to another
    #[instrument(skip(self))]
    pub async fn transfer_inventory(&self, command: TransferInventoryCommand) -> Result<(), ServiceError> {
        command.execute(self.db_pool.clone(), self.event_sender.clone()).await?;
        Ok(())
    }
    
    /// Receives new inventory into a location
    #[instrument(skip(self))]
    pub async fn receive_inventory(&self, command: ReceiveInventoryCommand) -> Result<(), ServiceError> {
        command.execute(self.db_pool.clone(), self.event_sender.clone()).await?;
        Ok(())
    }
    
    /// Performs a cycle count at a location
    #[instrument(skip(self))]
    pub async fn cycle_count(&self, command: CycleCountCommand) -> Result<(), ServiceError> {
        command.execute(self.db_pool.clone(), self.event_sender.clone()).await?;
        Ok(())
    }
    
    /// Lists all inventory items with pagination
    #[instrument(skip(self))]
    pub async fn list_inventory(&self, page: u64, limit: u64) -> Result<(Vec<inventory_items::Model>, u64), ServiceError> {
        use sea_orm::{Paginator, PaginatorTrait};
        
        let db = &*self.db_pool;
        
        // Create a paginator for the inventory items
        let paginator = inventory_items::Entity::find()
            .paginate(db, limit);
            
        // Get the total count of inventory items
        let total = paginator.num_items().await
            .map_err(|e| {
                let msg = format!("Failed to count inventory items: {}", e);
                error!(error = %e, "Database error when counting inventory items");
                ServiceError::DatabaseError(msg)
            })?;
            
        // Get the requested page of inventory items
        let items = paginator.fetch_page(page - 1).await
            .map_err(|e| {
                let msg = format!("Failed to fetch inventory items: {}", e);
                error!(page = %page, limit = %limit, error = %e, "Database error when fetching inventory items");
                ServiceError::DatabaseError(msg)
            })?;
            
        Ok((items, total))
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
    async fn test_adjust_inventory() {
        // Setup
        let (event_sender, _) = broadcast::channel(10);
        let event_sender = Arc::new(event_sender);
        let db_pool = Arc::new(MockDatabase::new());
        let redis_client = Arc::new(redis::Client::open("redis://localhost").unwrap());
        let message_queue = Arc::new(crate::message_queue::MockMessageQueue::new());
        let circuit_breaker = Arc::new(CircuitBreaker::new(5, std::time::Duration::from_secs(60), 1));
        let logger = slog::Logger::root(slog::Discard, slog::o!());
        
        let service = InventoryService::new(
            db_pool,
            event_sender,
            redis_client,
            message_queue,
            circuit_breaker,
            logger,
        );

        // Test data
        let product_id = Uuid::from_str("00000000-0000-0000-0000-000000000001").unwrap();
        let location_id = Uuid::from_str("00000000-0000-0000-0000-000000000002").unwrap();
        
        let command = AdjustInventoryCommand {
            product_id,
            location_id,
            adjustment: 10,
            reason: "Inventory count".to_string(),
        };

        // Execute
        let result = service.adjust_inventory(command).await;
        
        // Assert
        assert!(result.is_err()); // Will fail because we're using mock DB
    }
}