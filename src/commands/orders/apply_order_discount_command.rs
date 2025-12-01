use crate::{
    commands::Command,
    db::DbPool,
    errors::ServiceError,
    events::{Event, EventSender},
    models::order_entity::{self, Entity as Order},
};
use async_trait::async_trait;
use chrono::Utc;
use lazy_static::lazy_static;
use prometheus::IntCounter;
use sea_orm::{*, Set, IntoActiveModel};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info, instrument};
use uuid::Uuid;
use validator::Validate;

lazy_static! {
    static ref ORDER_DISCOUNTS_APPLIED: IntCounter = IntCounter::new(
        "order_discounts_applied_total",
        "Total number of discounts applied to orders"
    )
    .expect("metric can be created");
    static ref ORDER_DISCOUNT_FAILURES: IntCounter = IntCounter::new(
        "order_discount_failures_total",
        "Total number of failed order discount applications"
    )
    .expect("metric can be created");
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct ApplyOrderDiscountCommand {
    pub order_id: Uuid,

    #[validate(range(min = 0.01, message = "Discount amount must be greater than zero"))]
    pub discount_amount: f64,
}

#[async_trait::async_trait]
impl Command for ApplyOrderDiscountCommand {
    type Result = order_entity::Model;

    #[instrument(skip(db_pool, event_sender))]
    async fn execute(
        &self,
        db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        let db = db_pool.clone();

        let updated_order = self
            .apply_discount(&db, event_sender.clone())
            .await
            .map_err(|e| {
                ORDER_DISCOUNT_FAILURES.inc();
                error!(
                    "Failed to apply discount to order ID {}: {}",
                    self.order_id, e
                );
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
        event_sender: Arc<EventSender>,
    ) -> Result<order_entity::Model, ServiceError> {
        let order = order_entity::Entity::find_by_id(self.order_id)
            .one(db)
            .await
            .map_err(|e| ServiceError::db_error(e))?
            .ok_or_else(|| {
                let msg = format!("Order {} not found", self.order_id);
                error!("{}", msg);
                ServiceError::NotFound(msg)
            })?;

        let mut active_model: order_entity::ActiveModel = order.into();
        active_model.total_amount =
            Set(active_model.total_amount.unwrap_or_default() - self.discount_amount);

        let updated_order = active_model.update(db).await.map_err(|e| {
            let msg = format!("Failed to update Order {}: {}", self.order_id, e);
            error!("{}", msg);
            ServiceError::db_error(e)
        })?;

        event_sender
            .send(Event::OrderUpdated {
                order_id: self.order_id,
                checkout_session_id: None,
                status: None,
                refunds: vec![],
            })
            .await
            .map_err(|e| {
                ORDER_DISCOUNT_FAILURES.inc();
                error!(
                    "Failed to send OrderUpdated event for order ID {}: {}",
                    self.order_id, e
                );
                ServiceError::EventError(e.to_string())
            })?;

        Ok(updated_order)
    }
}
