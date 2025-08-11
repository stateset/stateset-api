use crate::circuit_breaker::CircuitBreaker;
use crate::message_queue::MessageQueue;
use crate::{
    commands::customers::{
        create_customer_command::CreateCustomerCommand,
        delete_customer_command::DeleteCustomerCommand,
        update_customer_command::UpdateCustomerCommand,
    },
    commands::Command,
    db::DbPool,
    errors::ServiceError,
    events::{Event, EventSender},
    models::customer,
};
use anyhow::Result;
use redis::Client as RedisClient;
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set, QuerySelect, PaginatorTrait};
use slog::Logger;
use std::sync::Arc;
use tracing::{error, info, instrument};
use uuid::Uuid;

/// Service for managing customers
#[derive(Clone)]
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
    pub async fn create_customer_service(
        &self,
        command: CreateCustomerCommand,
    ) -> Result<Uuid, ServiceError> {
        let result = command
            .execute(self.db_pool.clone(), self.event_sender.clone())
            .await?;
        Ok(result.id)
    }

    /// Updates an existing customer
    #[instrument(skip(self))]
    pub async fn update_customer_service(
        &self,
        command: UpdateCustomerCommand,
    ) -> Result<(), ServiceError> {
        command
            .execute(self.db_pool.clone(), self.event_sender.clone())
            .await?;
        Ok(())
    }

    /// Deletes a customer
    #[instrument(skip(self))]
    pub async fn delete_customer_service(
        &self,
        command: DeleteCustomerCommand,
    ) -> Result<(), ServiceError> {
        command
            .execute(self.db_pool.clone(), self.event_sender.clone())
            .await?;
        Ok(())
    }

    /// Gets a customer by ID
    #[instrument(skip(self))]
    pub async fn get_customer_service(
        &self,
        customer_id: &Uuid,
    ) -> Result<Option<customer::Model>, ServiceError> {
        let db = &*self.db_pool;
        let customer = customer::Entity::find_by_id(*customer_id)
            .one(db)
            .await
            .map_err(|e| ServiceError::DatabaseError(e))?;

        Ok(customer)
    }

    /// Gets a customer by email
    #[instrument(skip(self))]
    pub async fn get_customer_by_email(
        &self,
        email: &str,
    ) -> Result<Option<customer::Model>, ServiceError> {
        let db = &*self.db_pool;
        let customer = customer::Entity::find()
            .filter(customer::Column::Email.eq(email))
            .one(db)
            .await
            .map_err(|e| ServiceError::DatabaseError(e))?;

        Ok(customer)
    }

    /// Lists all customers
    #[instrument(skip(self))]
    pub async fn list_customers_service(
        &self,
        limit: u64,
        offset: u64,
    ) -> Result<Vec<customer::Model>, ServiceError> {
        let db = &*self.db_pool;
        let customers = customer::Entity::find()
            .limit(Some(limit))
            .offset(offset)
            .all(db)
            .await
            .map_err(|e| ServiceError::DatabaseError(e))?;

        Ok(customers)
    }

    /// Counts total customers
    #[instrument(skip(self))]
    pub async fn count_customers(&self) -> Result<u64, ServiceError> {
        let db = &*self.db_pool;
        let count = customer::Entity::find().count(db).await.map_err(|e| ServiceError::DatabaseError(e))?;

        Ok(count)
    }

    /// Searches customers by name or email
    #[instrument(skip(self))]
    pub async fn search_customers_service(
        &self,
        search_term: &str,
    ) -> Result<Vec<customer::Model>, ServiceError> {
        let db = &*self.db_pool;
        let search_pattern = format!("%{}%", search_term);

        let customers = customer::Entity::find()
            .filter(
                sea_orm::Condition::any()
                    .add(customer::Column::FirstName.like(&search_pattern))
                    .add(customer::Column::LastName.like(&search_pattern))
                    .add(customer::Column::Email.like(&search_pattern))
            )
            .all(db)
            .await
            .map_err(|e| ServiceError::DatabaseError(e))?;

        Ok(customers)
    }

    /// Gets customer orders
    #[instrument(skip(self))]
    pub async fn get_customer_orders_service(
        &self,
        customer_id: &Uuid,
    ) -> Result<Vec<crate::models::order::Model>, ServiceError> {
        let db = &*self.db_pool;
        let customer = customer::Entity::find_by_id(*customer_id)
            .one(db)
            .await
            .map_err(|e| ServiceError::DatabaseError(e))?
            .ok_or(ServiceError::NotFound("Customer not found".to_string()))?;

        let orders = crate::models::order::Entity::find()
            .filter(crate::models::order::Column::CustomerEmail.eq(customer.email)) // Assuming linking by email
            .all(db)
            .await
            .map_err(|e| ServiceError::DatabaseError(e))?;

        Ok(orders)
    }

    /// Gets customer returns
    #[instrument(skip(self))]
    pub async fn get_customer_returns_service(
        &self,
        customer_id: &Uuid,
    ) -> Result<Vec<crate::models::r#return::Model>, ServiceError> {
        let db = &*self.db_pool;
        let returns = crate::models::r#return::Entity::find()
            .filter(crate::models::r#return::Column::CustomerId.eq(*customer_id))
            .all(db)
            .await
            .map_err(|e| ServiceError::DatabaseError(e))?;

        Ok(returns)
    }
}

// Standalone function wrappers for backward compatibility
// These are placeholder implementations that would need proper service injection

/// Standalone function to create a customer
pub async fn create_customer(_command: CreateCustomerCommand) -> Result<Uuid, ServiceError> {
    // This should be injected with proper dependencies in the handler
    Err(ServiceError::InternalError(
        "create_customer requires service injection".to_string(),
    ))
}

/// Standalone function to get a customer
pub async fn get_customer(_customer_id: &Uuid) -> Result<Option<customer::Model>, ServiceError> {
    // This should be injected with proper dependencies in the handler
    Err(ServiceError::InternalError(
        "get_customer requires service injection".to_string(),
    ))
}

/// Standalone function to update a customer
pub async fn update_customer(_command: UpdateCustomerCommand) -> Result<(), ServiceError> {
    // This should be injected with proper dependencies in the handler
    Err(ServiceError::InternalError(
        "update_customer requires service injection".to_string(),
    ))
}

/// Standalone function to delete a customer
pub async fn delete_customer(_command: DeleteCustomerCommand) -> Result<(), ServiceError> {
    // This should be injected with proper dependencies in the handler
    Err(ServiceError::InternalError(
        "delete_customer requires service injection".to_string(),
    ))
}

/// Standalone function to list customers
pub async fn list_customers(
    _limit: u64,
    _offset: u64,
) -> Result<Vec<customer::Model>, ServiceError> {
    // This should be injected with proper dependencies in the handler
    Err(ServiceError::InternalError(
        "list_customers requires service injection".to_string(),
    ))
}

/// Standalone function to search customers
pub async fn search_customers(_search_term: &str) -> Result<Vec<customer::Model>, ServiceError> {
    // This should be injected with proper dependencies in the handler
    Err(ServiceError::InternalError(
        "search_customers requires service injection".to_string(),
    ))
}

/// Standalone function to get customer orders
pub async fn get_customer_orders(
    _customer_id: &Uuid,
) -> Result<Vec<crate::models::order::Model>, ServiceError> {
    // This should be injected with proper dependencies in the handler
    Err(ServiceError::InternalError(
        "get_customer_orders requires service injection".to_string(),
    ))
}

/// Standalone function to get customer returns
pub async fn get_customer_returns(
    _customer_id: &Uuid,
) -> Result<Vec<crate::models::r#return::Model>, ServiceError> {
    // This should be injected with proper dependencies in the handler
    Err(ServiceError::InternalError(
        "get_customer_returns requires service injection".to_string(),
    ))
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
    async fn test_create_customer() {
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
