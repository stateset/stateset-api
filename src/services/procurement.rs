use crate::circuit_breaker::CircuitBreaker;
use crate::commands::purchaseorders::{
    ApprovePurchaseOrderCommand, CancelPurchaseOrderCommand, CreatePurchaseOrderCommand,
    ReceivePurchaseOrderCommand, RejectPurchaseOrderCommand, SubmitPurchaseOrderCommand,
    UpdatePurchaseOrderCommand,
};
use crate::commands::purchaseorders::reject_purchase_order_command::RejectPurchaseOrderResult;
use crate::commands::purchaseorders::submit_purchase_order_command::SubmitPurchaseOrderResult;
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
    events::EventSender,
    models::purchase_order,
};
use anyhow::Result;
use chrono::NaiveDateTime;
use redis::Client as RedisClient;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use slog::Logger;
use std::sync::Arc;
use tracing::instrument;
use uuid::Uuid;

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

    /// Submits a purchase order for approval
    #[instrument(skip(self))]
    pub async fn submit_purchase_order(
        &self,
        command: SubmitPurchaseOrderCommand,
    ) -> Result<SubmitPurchaseOrderResult, ServiceError> {
        command
            .execute(self.db_pool.clone(), self.event_sender.clone())
            .await
    }

    /// Rejects a purchase order
    #[instrument(skip(self))]
    pub async fn reject_purchase_order(
        &self,
        command: RejectPurchaseOrderCommand,
    ) -> Result<RejectPurchaseOrderResult, ServiceError> {
        command
            .execute(self.db_pool.clone(), self.event_sender.clone())
            .await
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
            .map_err(|e| ServiceError::db_error(e))?;

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
            .map_err(|e| ServiceError::db_error(e))?;

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
            .map_err(|e| ServiceError::db_error(e))?;

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
            .map_err(|e| ServiceError::db_error(e))?;

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
            .map_err(|e| ServiceError::db_error(e))?;

        let total_value: f64 = pos
            .iter()
            .filter_map(|po| Some(po.total_amount.to_string().parse::<f64>().unwrap_or(0.0)))
            .sum();

        Ok(total_value)
    }
}

#[cfg(all(test, feature = "mock-tests"))]
mod tests {
    // Unit tests temporarily disabled; integration coverage exercises procurement paths.
}
