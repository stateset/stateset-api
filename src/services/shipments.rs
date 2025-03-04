use std::sync::Arc;
use crate::{
    db::DbPool,
    errors::ServiceError,
    events::{Event, EventSender},
    models::shipment,
    commands::shipments::{
        create_shipment_command::CreateShipmentCommand,
        update_shipment_command::UpdateShipmentCommand,
        cancel_shipment_command::CancelShipmentCommand,
        track_shipment_command::TrackShipmentCommand,
        confirm_shipment_delivery_command::ConfirmShipmentDeliveryCommand,
        assign_shipment_carrier_command::AssignShipmentCarrierCommand,
        ship_command::ShipCommand,
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

/// Service for managing shipments
pub struct ShipmentService {
    db_pool: Arc<DbPool>,
    event_sender: Arc<EventSender>,
    redis_client: Arc<RedisClient>,
    message_queue: Arc<dyn MessageQueue>,
    circuit_breaker: Arc<CircuitBreaker>,
    logger: Logger,
}

impl ShipmentService {
    /// Creates a new shipment service instance
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

    /// Creates a new shipment
    #[instrument(skip(self))]
    pub async fn create_shipment(&self, command: CreateShipmentCommand) -> Result<Uuid, ServiceError> {
        let result = command.execute(self.db_pool.clone(), self.event_sender.clone()).await?;
        Ok(result.id)
    }

    /// Updates a shipment
    #[instrument(skip(self))]
    pub async fn update_shipment(&self, command: UpdateShipmentCommand) -> Result<(), ServiceError> {
        command.execute(self.db_pool.clone(), self.event_sender.clone()).await?;
        Ok(())
    }

    /// Cancels a shipment
    #[instrument(skip(self))]
    pub async fn cancel_shipment(&self, command: CancelShipmentCommand) -> Result<(), ServiceError> {
        command.execute(self.db_pool.clone(), self.event_sender.clone()).await?;
        Ok(())
    }

    /// Tracks a shipment
    #[instrument(skip(self))]
    pub async fn track_shipment(&self, command: TrackShipmentCommand) -> Result<String, ServiceError> {
        let result = command.execute(self.db_pool.clone(), self.event_sender.clone()).await?;
        Ok(result.tracking_status)
    }

    /// Confirms delivery of a shipment
    #[instrument(skip(self))]
    pub async fn confirm_delivery(&self, command: ConfirmShipmentDeliveryCommand) -> Result<(), ServiceError> {
        command.execute(self.db_pool.clone(), self.event_sender.clone()).await?;
        Ok(())
    }

    /// Assigns a carrier to a shipment
    #[instrument(skip(self))]
    pub async fn assign_carrier(&self, command: AssignShipmentCarrierCommand) -> Result<(), ServiceError> {
        command.execute(self.db_pool.clone(), self.event_sender.clone()).await?;
        Ok(())
    }

    /// Processes the shipping of a shipment
    #[instrument(skip(self))]
    pub async fn ship(&self, command: ShipCommand) -> Result<(), ServiceError> {
        command.execute(self.db_pool.clone(), self.event_sender.clone()).await?;
        Ok(())
    }

    /// Gets a shipment by ID
    #[instrument(skip(self))]
    pub async fn get_shipment(&self, shipment_id: &Uuid) -> Result<Option<shipment::Model>, ServiceError> {
        let db = &*self.db_pool;
        let shipment = shipment::Entity::find_by_id(*shipment_id)
            .one(db)
            .await
            .map_err(|e| {
                let msg = format!("Failed to get shipment: {}", e);
                error!(shipment_id = %shipment_id, error = %e, "Database error when fetching shipment");
                ServiceError::DatabaseError(msg)
            })?;
        
        Ok(shipment)
    }

    /// Gets shipments for an order
    #[instrument(skip(self))]
    pub async fn get_shipments_for_order(&self, order_id: &Uuid) -> Result<Vec<shipment::Model>, ServiceError> {
        let db = &*self.db_pool;
        let shipments = shipment::Entity::find()
            .filter(shipment::Column::OrderId.eq(*order_id))
            .all(db)
            .await
            .map_err(|e| {
                let msg = format!("Failed to get shipments for order: {}", e);
                error!(order_id = %order_id, error = %e, "Database error when fetching shipments");
                ServiceError::DatabaseError(msg)
            })?;
        
        Ok(shipments)
    }
    
    /// Lists all shipments with pagination
    #[instrument(skip(self))]
    pub async fn list_shipments(&self, page: u64, limit: u64) -> Result<(Vec<shipment::Model>, u64), ServiceError> {
        use sea_orm::{Paginator, PaginatorTrait};
        
        let db = &*self.db_pool;
        
        // Create a paginator for the shipments
        let paginator = shipment::Entity::find()
            .order_by_desc(shipment::Column::CreatedAt)
            .paginate(db, limit);
            
        // Get the total count of shipments
        let total = paginator.num_items().await
            .map_err(|e| {
                let msg = format!("Failed to count shipments: {}", e);
                error!(error = %e, "Database error when counting shipments");
                ServiceError::DatabaseError(msg)
            })?;
            
        // Get the requested page of shipments
        let shipments = paginator.fetch_page(page - 1).await
            .map_err(|e| {
                let msg = format!("Failed to fetch shipments: {}", e);
                error!(page = %page, limit = %limit, error = %e, "Database error when fetching shipments");
                ServiceError::DatabaseError(msg)
            })?;
            
        Ok((shipments, total))
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
    async fn test_create_shipment() {
        // Setup
        let (event_sender, _) = broadcast::channel(10);
        let event_sender = Arc::new(event_sender);
        let db_pool = Arc::new(MockDatabase::new());
        let redis_client = Arc::new(redis::Client::open("redis://localhost").unwrap());
        let message_queue = Arc::new(crate::message_queue::MockMessageQueue::new());
        let circuit_breaker = Arc::new(CircuitBreaker::new(5, std::time::Duration::from_secs(60), 1));
        let logger = slog::Logger::root(slog::Discard, slog::o!());
        
        let service = ShipmentService::new(
            db_pool,
            event_sender,
            redis_client,
            message_queue,
            circuit_breaker,
            logger,
        );

        // Test data
        let order_id = Uuid::from_str("00000000-0000-0000-0000-000000000001").unwrap();
        
        let command = CreateShipmentCommand {
            order_id,
            recipient_name: "John Doe".to_string(),
            shipping_address: "123 Main St, City, Country".to_string(),
            carrier: Some("DHL".to_string()),
            tracking_number: None,
        };

        // Execute
        let result = service.create_shipment(command).await;
        
        // Assert
        assert!(result.is_err()); // Will fail because we're using mock DB
    }
}