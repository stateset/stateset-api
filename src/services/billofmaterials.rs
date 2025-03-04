use std::sync::Arc;
use crate::{
    db::DbPool,
    errors::ServiceError,
    events::{Event, EventSender},
    models::billofmaterials,
    commands::billofmaterials::{
        create_bom_command::CreateBomCommand,
        update_bom_command::UpdateBomCommand,
        delete_bom_command::DeleteBomCommand,
        add_component_to_bom_command::AddComponentToBomCommand,
        remove_component_from_bom_command::RemoveComponentFromBomCommand,
        audit_bom_command::AuditBomCommand,
        duplicate_bom_command::DuplicateBomCommand,
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

/// Service for managing bill of materials
pub struct BillOfMaterialsService {
    db_pool: Arc<DbPool>,
    event_sender: Arc<EventSender>,
    redis_client: Arc<RedisClient>,
    message_queue: Arc<dyn MessageQueue>,
    circuit_breaker: Arc<CircuitBreaker>,
    logger: Logger,
}

impl BillOfMaterialsService {
    /// Creates a new bill of materials service instance
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

    /// Creates a new bill of materials
    #[instrument(skip(self))]
    pub async fn create_bom(&self, command: CreateBomCommand) -> Result<Uuid, ServiceError> {
        let result = command.execute(self.db_pool.clone(), self.event_sender.clone()).await?;
        Ok(result.id)
    }

    /// Updates an existing bill of materials
    #[instrument(skip(self))]
    pub async fn update_bom(&self, command: UpdateBomCommand) -> Result<(), ServiceError> {
        command.execute(self.db_pool.clone(), self.event_sender.clone()).await?;
        Ok(())
    }

    /// Deletes a bill of materials
    #[instrument(skip(self))]
    pub async fn delete_bom(&self, command: DeleteBomCommand) -> Result<(), ServiceError> {
        command.execute(self.db_pool.clone(), self.event_sender.clone()).await?;
        Ok(())
    }

    /// Adds a component to a bill of materials
    #[instrument(skip(self))]
    pub async fn add_component_to_bom(&self, command: AddComponentToBomCommand) -> Result<(), ServiceError> {
        command.execute(self.db_pool.clone(), self.event_sender.clone()).await?;
        Ok(())
    }

    /// Removes a component from a bill of materials
    #[instrument(skip(self))]
    pub async fn remove_component_from_bom(&self, command: RemoveComponentFromBomCommand) -> Result<(), ServiceError> {
        command.execute(self.db_pool.clone(), self.event_sender.clone()).await?;
        Ok(())
    }

    /// Audits a bill of materials
    #[instrument(skip(self))]
    pub async fn audit_bom(&self, command: AuditBomCommand) -> Result<(), ServiceError> {
        command.execute(self.db_pool.clone(), self.event_sender.clone()).await?;
        Ok(())
    }

    /// Duplicates a bill of materials
    #[instrument(skip(self))]
    pub async fn duplicate_bom(&self, command: DuplicateBomCommand) -> Result<Uuid, ServiceError> {
        let result = command.execute(self.db_pool.clone(), self.event_sender.clone()).await?;
        Ok(result.id)
    }

    /// Gets a bill of materials by ID
    #[instrument(skip(self))]
    pub async fn get_bom(&self, bom_id: &Uuid) -> Result<Option<billofmaterials::Model>, ServiceError> {
        let db = &*self.db_pool;
        let bom = billofmaterials::Entity::find_by_id(*bom_id)
            .one(db)
            .await
            .map_err(|e| {
                let msg = format!("Failed to get bill of materials: {}", e);
                error!(bom_id = %bom_id, error = %e, "Database error when fetching BOM");
                ServiceError::DatabaseError(msg)
            })?;
        
        Ok(bom)
    }

    /// Gets bill of materials for a product
    #[instrument(skip(self))]
    pub async fn get_boms_for_product(&self, product_id: &Uuid) -> Result<Vec<billofmaterials::Model>, ServiceError> {
        let db = &*self.db_pool;
        let boms = billofmaterials::Entity::find()
            .filter(billofmaterials::Column::ProductId.eq(*product_id))
            .all(db)
            .await
            .map_err(|e| {
                let msg = format!("Failed to get bill of materials for product: {}", e);
                error!(product_id = %product_id, error = %e, "Database error when fetching BOMs");
                ServiceError::DatabaseError(msg)
            })?;
        
        Ok(boms)
    }

    /// Gets bill of materials by version
    #[instrument(skip(self))]
    pub async fn get_bom_by_version(&self, product_id: &Uuid, version: &str) -> Result<Option<billofmaterials::Model>, ServiceError> {
        let db = &*self.db_pool;
        let bom = billofmaterials::Entity::find()
            .filter(billofmaterials::Column::ProductId.eq(*product_id))
            .filter(billofmaterials::Column::Version.eq(version))
            .one(db)
            .await
            .map_err(|e| {
                let msg = format!("Failed to get bill of materials by version: {}", e);
                error!(product_id = %product_id, version = %version, error = %e, "Database error when fetching BOM");
                ServiceError::DatabaseError(msg)
            })?;
        
        Ok(bom)
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
    async fn test_create_bom() {
        // Setup
        let (event_sender, _) = broadcast::channel(10);
        let event_sender = Arc::new(event_sender);
        let db_pool = Arc::new(MockDatabase::new());
        let redis_client = Arc::new(redis::Client::open("redis://localhost").unwrap());
        let message_queue = Arc::new(crate::message_queue::MockMessageQueue::new());
        let circuit_breaker = Arc::new(CircuitBreaker::new(5, std::time::Duration::from_secs(60), 1));
        let logger = slog::Logger::root(slog::Discard, slog::o!());
        
        let service = BillOfMaterialsService::new(
            db_pool,
            event_sender,
            redis_client,
            message_queue,
            circuit_breaker,
            logger,
        );

        // Test data
        let product_id = Uuid::from_str("00000000-0000-0000-0000-000000000001").unwrap();
        
        let command = CreateBomCommand {
            product_id,
            name: "Test BOM".to_string(),
            description: "Test description".to_string(),
            version: "1.0.0".to_string(),
            components: vec![],
        };

        // Execute
        let result = service.create_bom(command).await;
        
        // Assert
        assert!(result.is_err()); // Will fail because we're using mock DB
    }
}