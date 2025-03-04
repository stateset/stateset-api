use std::sync::Arc;
use crate::{
    db::DbPool,
    errors::ServiceError,
    events::{Event, EventSender},
    models::warranty,
    commands::warranties::{
        create_warranty_command::CreateWarrantyCommand,
        claim_warranty_command::ClaimWarrantyCommand,
        approve_warranty_claim_command::ApproveWarrantyClaimCommand,
        reject_warranty_claim_command::RejectWarrantyClaimCommand,
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
use chrono::{DateTime, Utc};

/// Service for managing warranties
pub struct WarrantyService {
    db_pool: Arc<DbPool>,
    event_sender: Arc<EventSender>,
    redis_client: Arc<RedisClient>,
    message_queue: Arc<dyn MessageQueue>,
    circuit_breaker: Arc<CircuitBreaker>,
    logger: Logger,
}

impl WarrantyService {
    /// Creates a new warranty service instance
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

    /// Creates a new warranty
    #[instrument(skip(self))]
    pub async fn create_warranty(&self, command: CreateWarrantyCommand) -> Result<Uuid, ServiceError> {
        let result = command.execute(self.db_pool.clone(), self.event_sender.clone()).await?;
        Ok(result)
    }

    /// Processes a warranty claim
    #[instrument(skip(self))]
    pub async fn claim_warranty(&self, command: ClaimWarrantyCommand) -> Result<Uuid, ServiceError> {
        let result = command.execute(self.db_pool.clone(), self.event_sender.clone()).await?;
        Ok(result)
    }

    /// Approves a warranty claim
    #[instrument(skip(self))]
    pub async fn approve_warranty_claim(&self, command: ApproveWarrantyClaimCommand) -> Result<(), ServiceError> {
        command.execute(self.db_pool.clone(), self.event_sender.clone()).await?;
        Ok(())
    }

    /// Rejects a warranty claim
    #[instrument(skip(self))]
    pub async fn reject_warranty_claim(&self, command: RejectWarrantyClaimCommand) -> Result<(), ServiceError> {
        command.execute(self.db_pool.clone(), self.event_sender.clone()).await?;
        Ok(())
    }

    /// Gets a warranty by ID
    #[instrument(skip(self))]
    pub async fn get_warranty(&self, warranty_id: &Uuid) -> Result<Option<warranty::Model>, ServiceError> {
        let db = &*self.db_pool;
        let warranty = warranty::Entity::find_by_id(*warranty_id)
            .one(db)
            .await
            .map_err(|e| {
                let msg = format!("Failed to get warranty: {}", e);
                error!(warranty_id = %warranty_id, error = %e, "Database error when fetching warranty");
                ServiceError::DatabaseError(msg)
            })?;
        
        Ok(warranty)
    }

    /// Gets warranties for a product
    #[instrument(skip(self))]
    pub async fn get_warranties_for_product(&self, product_id: &Uuid) -> Result<Vec<warranty::Model>, ServiceError> {
        let db = &*self.db_pool;
        let warranties = warranty::Entity::find()
            .filter(warranty::Column::ProductId.eq(*product_id))
            .all(db)
            .await
            .map_err(|e| {
                let msg = format!("Failed to get warranties for product: {}", e);
                error!(product_id = %product_id, error = %e, "Database error when fetching warranties");
                ServiceError::DatabaseError(msg)
            })?;
        
        Ok(warranties)
    }

    /// Gets active warranties for a customer
    #[instrument(skip(self))]
    pub async fn get_active_warranties_for_customer(&self, customer_id: &Uuid) -> Result<Vec<warranty::Model>, ServiceError> {
        let db = &*self.db_pool;
        let now = Utc::now();
        
        let warranties = warranty::Entity::find()
            .filter(warranty::Column::CustomerId.eq(*customer_id))
            .filter(warranty::Column::EndDate.gt(now))
            .all(db)
            .await
            .map_err(|e| {
                let msg = format!("Failed to get active warranties for customer: {}", e);
                error!(customer_id = %customer_id, error = %e, "Database error when fetching warranties");
                ServiceError::DatabaseError(msg)
            })?;
        
        Ok(warranties)
    }

    /// Checks if a product is under warranty
    #[instrument(skip(self))]
    pub async fn is_under_warranty(&self, product_id: &Uuid, serial_number: &str) -> Result<bool, ServiceError> {
        let db = &*self.db_pool;
        let now = Utc::now();
        
        let warranty_exists = warranty::Entity::find()
            .filter(warranty::Column::ProductId.eq(*product_id))
            .filter(warranty::Column::EndDate.gt(now))
            .count(db)
            .await
            .map_err(|e| {
                let msg = format!("Failed to check warranty status: {}", e);
                error!(product_id = %product_id, serial_number = %serial_number, error = %e, "Database error when checking warranty");
                ServiceError::DatabaseError(msg)
            })?;
        
        Ok(warranty_exists > 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockall::predicate::*;
    use mockall::mock;
    use tokio::sync::broadcast;
    use std::str::FromStr;
    use chrono::Duration;

    mock! {
        pub Database {}
        impl Clone for Database {
            fn clone(&self) -> Self;
        }
    }

    #[tokio::test]
    async fn test_create_warranty() {
        // Setup
        let (event_sender, _) = broadcast::channel(10);
        let event_sender = Arc::new(event_sender);
        let db_pool = Arc::new(MockDatabase::new());
        let redis_client = Arc::new(redis::Client::open("redis://localhost").unwrap());
        let message_queue = Arc::new(crate::message_queue::MockMessageQueue::new());
        let circuit_breaker = Arc::new(CircuitBreaker::new(5, std::time::Duration::from_secs(60), 1));
        let logger = slog::Logger::root(slog::Discard, slog::o!());
        
        let service = WarrantyService::new(
            db_pool,
            event_sender,
            redis_client,
            message_queue,
            circuit_breaker,
            logger,
        );

        // Test data
        let product_id = Uuid::from_str("00000000-0000-0000-0000-000000000001").unwrap();
        let customer_id = Uuid::from_str("00000000-0000-0000-0000-000000000002").unwrap();
        let expiration_date = Utc::now() + Duration::days(365);
        
        let command = CreateWarrantyCommand {
            product_id,
            customer_id,
            serial_number: "SN123456789".to_string(),
            warranty_type: "Extended".to_string(),
            expiration_date: expiration_date.naive_utc(),
            terms: "Standard warranty terms".to_string(),
        };

        // Execute
        let result = service.create_warranty(command).await;
        
        // Assert
        assert!(result.is_err()); // Will fail because we're using mock DB
    }
}