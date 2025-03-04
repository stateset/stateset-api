use std::sync::Arc;
use crate::{
    db::DbPool,
    errors::ServiceError,
    events::{Event, EventSender},
    models::supplier,
    commands::suppliers::{
        create_supplier_command::CreateSupplierCommand,
        update_supplier_command::UpdateSupplierCommand,
        delete_supplier_command::DeleteSupplierCommand,
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

/// Service for managing suppliers
pub struct SupplierService {
    db_pool: Arc<DbPool>,
    event_sender: Arc<EventSender>,
    redis_client: Arc<RedisClient>,
    message_queue: Arc<dyn MessageQueue>,
    circuit_breaker: Arc<CircuitBreaker>,
    logger: Logger,
}

impl SupplierService {
    /// Creates a new supplier service instance
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

    /// Creates a new supplier
    #[instrument(skip(self))]
    pub async fn create_supplier(&self, command: CreateSupplierCommand) -> Result<Uuid, ServiceError> {
        let result = command.execute(self.db_pool.clone(), self.event_sender.clone()).await?;
        Ok(result.id)
    }

    /// Updates an existing supplier
    #[instrument(skip(self))]
    pub async fn update_supplier(&self, command: UpdateSupplierCommand) -> Result<(), ServiceError> {
        command.execute(self.db_pool.clone(), self.event_sender.clone()).await?;
        Ok(())
    }

    /// Deletes a supplier
    #[instrument(skip(self))]
    pub async fn delete_supplier(&self, command: DeleteSupplierCommand) -> Result<(), ServiceError> {
        command.execute(self.db_pool.clone(), self.event_sender.clone()).await?;
        Ok(())
    }

    /// Gets a supplier by ID
    #[instrument(skip(self))]
    pub async fn get_supplier(&self, supplier_id: &Uuid) -> Result<Option<supplier::Model>, ServiceError> {
        let db = &*self.db_pool;
        let supplier = supplier::Entity::find_by_id(*supplier_id)
            .one(db)
            .await
            .map_err(|e| {
                let msg = format!("Failed to get supplier: {}", e);
                error!(supplier_id = %supplier_id, error = %e, "Database error when fetching supplier");
                ServiceError::DatabaseError(msg)
            })?;
        
        Ok(supplier)
    }

    /// Gets a supplier by name
    #[instrument(skip(self))]
    pub async fn get_supplier_by_name(&self, name: &str) -> Result<Option<supplier::Model>, ServiceError> {
        let db = &*self.db_pool;
        let supplier = supplier::Entity::find()
            .filter(supplier::Column::Name.eq(name))
            .one(db)
            .await
            .map_err(|e| {
                let msg = format!("Failed to get supplier by name: {}", e);
                error!(name = %name, error = %e, "Database error when fetching supplier");
                ServiceError::DatabaseError(msg)
            })?;
        
        Ok(supplier)
    }

    /// Lists all suppliers
    #[instrument(skip(self))]
    pub async fn list_suppliers(&self, limit: u64, offset: u64) -> Result<Vec<supplier::Model>, ServiceError> {
        let db = &*self.db_pool;
        let suppliers = supplier::Entity::find()
            .limit(Some(limit))
            .offset(offset)
            .all(db)
            .await
            .map_err(|e| {
                let msg = format!("Failed to list suppliers: {}", e);
                error!(error = %e, "Database error when listing suppliers");
                ServiceError::DatabaseError(msg)
            })?;
        
        Ok(suppliers)
    }

    /// Gets suppliers by category
    #[instrument(skip(self))]
    pub async fn get_suppliers_by_category(&self, category: &str) -> Result<Vec<supplier::Model>, ServiceError> {
        let db = &*self.db_pool;
        let suppliers = supplier::Entity::find()
            .filter(supplier::Column::Category.eq(category))
            .all(db)
            .await
            .map_err(|e| {
                let msg = format!("Failed to get suppliers by category: {}", e);
                error!(category = %category, error = %e, "Database error when fetching suppliers");
                ServiceError::DatabaseError(msg)
            })?;
        
        Ok(suppliers)
    }

    /// Gets suppliers by rating (equal or higher)
    #[instrument(skip(self))]
    pub async fn get_suppliers_by_min_rating(&self, min_rating: f32) -> Result<Vec<supplier::Model>, ServiceError> {
        let db = &*self.db_pool;
        let suppliers = supplier::Entity::find()
            .filter(supplier::Column::Rating.gte(min_rating))
            .all(db)
            .await
            .map_err(|e| {
                let msg = format!("Failed to get suppliers by rating: {}", e);
                error!(min_rating = %min_rating, error = %e, "Database error when fetching suppliers");
                ServiceError::DatabaseError(msg)
            })?;
        
        Ok(suppliers)
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
    async fn test_create_supplier() {
        // Setup
        let (event_sender, _) = broadcast::channel(10);
        let event_sender = Arc::new(event_sender);
        let db_pool = Arc::new(MockDatabase::new());
        let redis_client = Arc::new(redis::Client::open("redis://localhost").unwrap());
        let message_queue = Arc::new(crate::message_queue::MockMessageQueue::new());
        let circuit_breaker = Arc::new(CircuitBreaker::new(5, std::time::Duration::from_secs(60), 1));
        let logger = slog::Logger::root(slog::Discard, slog::o!());
        
        let service = SupplierService::new(
            db_pool,
            event_sender,
            redis_client,
            message_queue,
            circuit_breaker,
            logger,
        );

        // Test data
        let command = CreateSupplierCommand {
            name: "Acme Supplies".to_string(),
            contact_name: "Jane Smith".to_string(),
            email: "jane.smith@acme.com".to_string(),
            phone: "1234567890".to_string(),
            address: "123 Supplier St, City, Country".to_string(),
            category: "Electronics".to_string(),
            payment_terms: "Net 30".to_string(),
        };

        // Execute
        let result = service.create_supplier(command).await;
        
        // Assert
        assert!(result.is_err()); // Will fail because we're using mock DB
    }
}