use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info, instrument};
use validator::Validate;

use crate::{
    db::DbPool,
    errors::ServiceError,
    events::{Event, EventSender},
    models::order_entity::{self, Entity as Order},
};
use sea_orm::{ActiveModelTrait, ActiveValue::Set, DatabaseConnection, EntityTrait};
use prometheus::IntCounter;
use lazy_static::lazy_static;
use uuid::Uuid;

lazy_static! {
    static ref PAYMENT_METHOD_UPDATES: IntCounter =
        IntCounter::new("payment_method_updates_total", "Total number of payment method updates")
            .expect("metric can be created");

    static ref PAYMENT_METHOD_UPDATE_FAILURES: IntCounter =
        IntCounter::new("payment_method_update_failures_total", "Total number of failed payment method updates")
            .expect("metric can be created");
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct UpdatePaymentMethodCommand {
    pub order_id: Uuid,

    #[validate(length(min = 1, max = 100))]
    pub new_method: String,
}

#[async_trait]
impl Command for UpdatePaymentMethodCommand {
    type Result = order_entity::Model;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        let db = db_pool.as_ref();

        let updated_order = self.update_payment_method(db).await?;

        self.log_and_trigger_event(event_sender, &updated_order).await?;

        PAYMENT_METHOD_UPDATES.inc();
        info!(order_id = %self.order_id, "Payment method updated successfully");

        Ok(updated_order)
    }
}

impl UpdatePaymentMethodCommand {
    async fn update_payment_method(&self, db: &DatabaseConnection) -> Result<order_entity::Model, ServiceError> {
        let order = Order::find_by_id(self.order_id)
            .one(db)
            .await
            .map_err(|e| {
                PAYMENT_METHOD_UPDATE_FAILURES.inc();
                let msg = format!("Failed to find order ID {}: {}", self.order_id, e);
                error!("{}", msg);
                ServiceError::DatabaseError(msg)
            })?
            .ok_or_else(|| {
                PAYMENT_METHOD_UPDATE_FAILURES.inc();
                let msg = format!("Order ID {} not found", self.order_id);
                error!("{}", msg);
                ServiceError::NotFoundError(msg)
            })?;

        let mut order_active_model: order_entity::ActiveModel = order.into();
        order_active_model.payment_method = Set(self.new_method.clone());
        order_active_model.updated_at = Set(chrono::Utc::now().naive_utc());

        order_active_model
            .update(db)
            .await
            .map_err(|e| {
                PAYMENT_METHOD_UPDATE_FAILURES.inc();
                let msg = format!("Failed to update payment method for order ID {}: {}", self.order_id, e);
                error!("{}", msg);
                ServiceError::DatabaseError(msg)
            })
    }

    async fn log_and_trigger_event(&self, event_sender: Arc<EventSender>, _updated_order: &order_entity::Model) -> Result<(), ServiceError> {
        event_sender
            .send(Event::PaymentMethodUpdated(self.order_id))
            .await
            .map_err(|e| {
                PAYMENT_METHOD_UPDATE_FAILURES.inc();
                error!("Failed to send PaymentMethodUpdated event for order ID {}: {}", self.order_id, e);
                ServiceError::EventError(e)
            })
    }
}
