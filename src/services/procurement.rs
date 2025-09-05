use crate::circuit_breaker::CircuitBreaker;
use crate::message_queue::MessageQueue;
use crate::{
    // commands::purchaseorders::{
        // approve_purchase_order_command::ApprovePurchaseOrderCommand,
        // cancel_purchase_order_command::CancelPurchaseOrderCommand,
        // create_purchase_order_command::CreatePurchaseOrderCommand,
        // receive_purchase_order_command::ReceivePurchaseOrderCommand,
        // update_purchase_order_command::UpdatePurchaseOrderCommand,
    // },
    commands::Command,
    db::DbPool,
    errors::ServiceError,
    events::{Event, EventSender},
    models::{purchase_order, supplier},
};
use crate::commands::purchaseorders::{
    ApprovePurchaseOrderCommand,
    CancelPurchaseOrderCommand,
    CreatePurchaseOrderCommand,
    ReceivePurchaseOrderCommand,
    UpdatePurchaseOrderCommand,
};
use anyhow::Result;
use chrono::NaiveDateTime;
use redis::Client as RedisClient;
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};
use slog::Logger;
use std::sync::Arc;
use tracing::{error, info, instrument};
use uuid::Uuid;
use rust_decimal::Decimal;

/// Service for managing procurement processes
#[derive(Clone)]
pub struct ProcurementService {
    db_pool: Arc<DbPool>,
    event_sender: Arc<EventSender>,
    redis_client: Arc<RedisClient>,
    message_queue: Arc<dyn MessageQueue>,
    circuit_breaker: Arc<CircuitBreaker>,
    logger: Logger,
}

impl ProcurementService {
    /// Creates a new procurement service instance
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

    /// Creates a new purchase order
    #[instrument(skip(self))]
    pub async fn create_purchase_order(
        &self,
        command: CreatePurchaseOrderCommand,
    ) -> Result<Uuid, ServiceError> {
        let result = command
            .execute(self.db_pool.clone(), self.event_sender.clone())
            .await?;
        Ok(result.id)
    }

    /// Updates an existing purchase order
    #[instrument(skip(self))]
    pub async fn update_purchase_order(
        &self,
        command: UpdatePurchaseOrderCommand,
    ) -> Result<(), ServiceError> {
        command
            .execute(self.db_pool.clone(), self.event_sender.clone())
            .await?;
        Ok(())
    }

    /// Approves a purchase order
    #[instrument(skip(self))]
    pub async fn approve_purchase_order(
        &self,
        command: ApprovePurchaseOrderCommand,
    ) -> Result<(), ServiceError> {
        command
            .execute(self.db_pool.clone(), self.event_sender.clone())
            .await?;
        Ok(())
    }

    /// Cancels a purchase order
    #[instrument(skip(self))]
    pub async fn cancel_purchase_order(
        &self,
        command: CancelPurchaseOrderCommand,
    ) -> Result<(), ServiceError> {
        command
            .execute(self.db_pool.clone(), self.event_sender.clone())
            .await?;
        Ok(())
    }

    /// Marks a purchase order as received
    #[instrument(skip(self))]
    pub async fn receive_purchase_order(
        &self,
        command: ReceivePurchaseOrderCommand,
    ) -> Result<(), ServiceError> {
        command
            .execute(self.db_pool.clone(), self.event_sender.clone())
            .await?;
        Ok(())
    }

    /// Gets a purchase order by ID
    #[instrument(skip(self))]
    pub async fn get_purchase_order(
        &self,
        po_id: &Uuid,
    ) -> Result<Option<purchase_order::Model>, ServiceError> {
        let db = &*self.db_pool;
        let po = purchase_order::Entity::find_by_id(*po_id)
            .one(db)
            .await
            .map_err(|e| ServiceError::DatabaseError(e))?;

        Ok(po)
    }

    /// Gets purchase orders for a supplier
    #[instrument(skip(self))]
    pub async fn get_purchase_orders_by_supplier(
        &self,
        supplier_id: &Uuid,
    ) -> Result<Vec<purchase_order::Model>, ServiceError> {
        let db = &*self.db_pool;
        let pos = purchase_order::Entity::find()
            .filter(purchase_order::Column::SupplierId.eq(*supplier_id))
            .all(db)
            .await
            .map_err(|e| ServiceError::DatabaseError(e))?;

        Ok(pos)
    }

    /// Gets purchase orders by status
    #[instrument(skip(self))]
    pub async fn get_purchase_orders_by_status(
        &self,
        status: &str,
    ) -> Result<Vec<purchase_order::Model>, ServiceError> {
        let db = &*self.db_pool;
        let pos = purchase_order::Entity::find()
            .filter(purchase_order::Column::Status.eq(status))
            .all(db)
            .await
            .map_err(|e| ServiceError::DatabaseError(e))?;

        Ok(pos)
    }

    /// Gets purchase orders due for delivery within a date range
    #[instrument(skip(self))]
    pub async fn get_purchase_orders_by_delivery_date(
        &self,
        start_date: NaiveDateTime,
        end_date: NaiveDateTime,
    ) -> Result<Vec<purchase_order::Model>, ServiceError> {
        let db = &*self.db_pool;
        let pos = purchase_order::Entity::find()
            .filter(purchase_order::Column::ExpectedDeliveryDate.gte(start_date))
            .filter(purchase_order::Column::ExpectedDeliveryDate.lte(end_date))
            .all(db)
            .await
            .map_err(|e| ServiceError::DatabaseError(e))?;

        Ok(pos)
    }

    /// Gets the total value of all purchase orders in a date range
    #[instrument(skip(self))]
    pub async fn get_total_purchase_value(
        &self,
        start_date: NaiveDateTime,
        end_date: NaiveDateTime,
    ) -> Result<f64, ServiceError> {
        let db = &*self.db_pool;
        let pos = purchase_order::Entity::find()
            .filter(purchase_order::Column::CreatedAt.gte(start_date))
            .filter(purchase_order::Column::CreatedAt.lte(end_date))
            .all(db)
            .await
            .map_err(|e| ServiceError::DatabaseError(e))?;

        let total_value: f64 = pos.iter().filter_map(|po| Some(po.total_amount.to_string().parse::<f64>().unwrap_or(0.0))).sum();

        Ok(total_value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
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
    async fn test_create_purchase_order() {
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

        let service = ProcurementService::new(
            db_pool,
            event_sender,
            redis_client,
            message_queue,
            circuit_breaker,
            logger,
        );

        // Test data
        let supplier_id = Uuid::from_str("00000000-0000-0000-0000-000000000001").unwrap();
        let expected_delivery = Utc::now().date_naive().and_hms_opt(0, 0, 0).unwrap();

        let command = CreatePurchaseOrderCommand {
            supplier_id,
            expected_delivery_date: expected_delivery,
            items: vec![],
            shipping_address: "123 Warehouse St, City, Country".to_string(),
            notes: None,
        };

        // Execute
        let result = service.create_purchase_order(command).await;

        // Assert
        assert!(result.is_err()); // Will fail because we're using mock DB
    }
}
