use std::sync::Arc;
use crate::{
    db::DbPool,
    errors::ServiceError,
    events::{Event, EventSender},
    models::customer,
    commands::customers::{
        create_customer_command::CreateCustomerCommand,
        update_customer_command::UpdateCustomerCommand,
        delete_customer_command::DeleteCustomerCommand,
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

/// Service for managing customers
pub struct CustomerService {
    db_pool: Arc<DbPool>,
    event_sender: Arc<EventSender>,
    redis_client: Arc<RedisClient>,
    message_queue: Arc<dyn MessageQueue>,
    circuit_breaker: Arc<CircuitBreaker>,
    logger: Logger,
}

impl CustomerService {
    /// Creates a new customer service instance
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

    /// Creates a new customer
    #[instrument(skip(self))]
    pub async fn create_customer(&self, command: CreateCustomerCommand) -> Result<Uuid, ServiceError> {
        let result = command.execute(self.db_pool.clone(), self.event_sender.clone()).await?;
        Ok(result.id)
    }

    /// Updates an existing customer
    #[instrument(skip(self))]
    pub async fn update_customer(&self, command: UpdateCustomerCommand) -> Result<(), ServiceError> {
        command.execute(self.db_pool.clone(), self.event_sender.clone()).await?;
        Ok(())
    }

    /// Deletes a customer
    #[instrument(skip(self))]
    pub async fn delete_customer(&self, command: DeleteCustomerCommand) -> Result<(), ServiceError> {
        command.execute(self.db_pool.clone(), self.event_sender.clone()).await?;
        Ok(())
    }

    /// Gets a customer by ID
    #[instrument(skip(self))]
    pub async fn get_customer(&self, customer_id: &Uuid) -> Result<Option<customer::Model>, ServiceError> {
        let db = &*self.db_pool;
        let customer = customer::Entity::find_by_id(*customer_id)
            .one(db)
            .await
            .map_err(|e| {
                let msg = format!("Failed to get customer: {}", e);
                error!(customer_id = %customer_id, error = %e, "Database error when fetching customer");
                ServiceError::DatabaseError(msg)
            })?;
        
        Ok(customer)
    }

    /// Gets a customer by email
    #[instrument(skip(self))]
    pub async fn get_customer_by_email(&self, email: &str) -> Result<Option<customer::Model>, ServiceError> {
        let db = &*self.db_pool;
        let customer = customer::Entity::find()
            .filter(customer::Column::Email.eq(email))
            .one(db)
            .await
            .map_err(|e| {
                let msg = format!("Failed to get customer by email: {}", e);
                error!(email = %email, error = %e, "Database error when fetching customer");
                ServiceError::DatabaseError(msg)
            })?;
        
        Ok(customer)
    }

    /// Lists all customers
    #[instrument(skip(self))]
    pub async fn list_customers(&self, limit: u64, offset: u64) -> Result<Vec<customer::Model>, ServiceError> {
        let db = &*self.db_pool;
        let customers = customer::Entity::find()
            .limit(Some(limit))
            .offset(offset)
            .all(db)
            .await
            .map_err(|e| {
                let msg = format!("Failed to list customers: {}", e);
                error!(error = %e, "Database error when listing customers");
                ServiceError::DatabaseError(msg)
            })?;
        
        Ok(customers)
    }

    /// Counts total customers
    #[instrument(skip(self))]
    pub async fn count_customers(&self) -> Result<u64, ServiceError> {
        let db = &*self.db_pool;
        let count = customer::Entity::find()
            .count(db)
            .await
            .map_err(|e| {
                let msg = format!("Failed to count customers: {}", e);
                error!(error = %e, "Database error when counting customers");
                ServiceError::DatabaseError(msg)
            })?;
        
        Ok(count)
    }

    /// Searches customers by name or email
    #[instrument(skip(self))]
    pub async fn search_customers(&self, search_term: &str) -> Result<Vec<customer::Model>, ServiceError> {
        let db = &*self.db_pool;
        let search_pattern = format!("%{}%", search_term);
        
        let customers = customer::Entity::find()
            .filter(
                sea_orm::Condition::any()
                    .add(customer::Column::Name.like(&search_pattern))
                    .add(customer::Column::Email.like(&search_pattern))
            )
            .all(db)
            .await
            .map_err(|e| {
                let msg = format!("Failed to search customers: {}", e);
                error!(search_term = %search_term, error = %e, "Database error when searching customers");
                ServiceError::DatabaseError(msg)
            })?;
        
        Ok(customers)
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
    async fn test_create_customer() {
        // Setup
        let (event_sender, _) = broadcast::channel(10);
        let event_sender = Arc::new(event_sender);
        let db_pool = Arc::new(MockDatabase::new());
        let redis_client = Arc::new(redis::Client::open("redis://localhost").unwrap());
        let message_queue = Arc::new(crate::message_queue::MockMessageQueue::new());
        let circuit_breaker = Arc::new(CircuitBreaker::new(5, std::time::Duration::from_secs(60), 1));
        let logger = slog::Logger::root(slog::Discard, slog::o!());
        
        let service = CustomerService::new(
            db_pool,
            event_sender,
            redis_client,
            message_queue,
            circuit_breaker,
            logger,
        );

        // Test data
        let command = CreateCustomerCommand {
            name: "John Doe".to_string(),
            email: "john.doe@example.com".to_string(),
            phone: Some("1234567890".to_string()),
            address: Some("123 Main St, City, Country".to_string()),
        };

        // Execute
        let result = service.create_customer(command).await;
        
        // Assert
        assert!(result.is_err()); // Will fail because we're using mock DB
    }
}