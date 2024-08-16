use async_trait::async_trait;;
use serde::{Serialize, Deserialize};
use std::sync::Arc;
use crate::{errors::ServiceError, db::DbPool, models::order_entity};
use crate::events::{Event, EventSender};
use validator::Validate;
use tracing::{info, error, instrument};
use sea_orm::{DatabaseConnection, EntityTrait, ActiveModelTrait, Set};
use prometheus::IntCounter;

lazy_static! {
    static ref ORDER_DISCOUNTS_APPLIED: IntCounter = 
        IntCounter::new("order_discounts_applied_total", "Total number of discounts applied to orders")
            .expect("metric can be created");

    static ref ORDER_DISCOUNT_FAILURES: IntCounter = 
        IntCounter::new("order_discount_failures_total", "Total number of failed order discount applications")
            .expect("metric can be created");
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct ApplyOrderDiscountCommand {
    #[validate(range(min = 1))]
    pub order_id: i32,

    #[validate(range(min = 0.01, message = "Discount amount must be greater than zero"))]
    pub discount_amount: f64,
}

#[async_trait::async_trait]
impl Command for ApplyOrderDiscountCommand {
    type Result = order_entity::Model;

    #[instrument(skip(db_pool, event_sender))]
    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        let db = db_pool.clone();

        let updated_order = self.apply_discount(&db, event_sender.clone()).await.map_err(|e| {
            ORDER_DISCOUNT_FAILURES.inc();
            error!("Failed to apply discount to order ID {}: {}", self.order_id, e);
            e
        })?;

        ORDER_DISCOUNTS_APPLIED.inc();

        info!(
            order_id = %self.order_id,
            discount_amount = %self.discount_amount,
            "Successfully applied discount to order"
        );

        Ok(updated_order)
    }
}

impl ApplyOrderDiscountCommand {
    async fn apply_discount(
        &self, 
        db: &DatabaseConnection, 
        event_sender: Arc<EventSender>
    ) -> Result<order_entity::Model, ServiceError> {
        let order = order_entity::Entity::find_by_id(self.order_id)
            .one(db)
            .await
            .map_err(|e| {
                ORDER_DISCOUNT_FAILURES.inc();
                error!("Failed to find Order ID {}: {}", self.order_id, e);
                ServiceError::DatabaseError(format!("Failed to find Order: {}", e))
            })?
            .ok_or_else(|| {
                ORDER_DISCOUNT_FAILURES.inc();
                error!("Order ID {} not found", self.order_id);
                ServiceError::NotFound(format!("Order ID {} not found", self.order_id))
            })?;

        let mut active_model = order.into_active_model();
        active_model.total_amount = Set(active_model.total_amount.unwrap_or_default() - self.discount_amount);

        let updated_order = active_model.update(db)
            .await
            .map_err(|e| {
                ORDER_DISCOUNT_FAILURES.inc();
                error!("Failed to update Order ID {}: {}", self.order_id, e);
                ServiceError::DatabaseError(format!("Failed to update Order: {}", e))
            })?;

        event_sender.send(Event::OrderUpdated(self.order_id))
            .await
            .map_err(|e| {
                ORDER_DISCOUNT_FAILURES.inc();
                error!("Failed to send OrderUpdated event for order ID {}: {}", self.order_id, e);
                ServiceError::EventError(e.to_string())
            })?;

        Ok(updated_order)
    }
}
