use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info, instrument};
use validator::Validate;

use crate::{
    commands::Command,
    db::DbPool,
    errors::ServiceError,
    events::{Event, EventSender},
    models::order_entity::{self, Entity as Order},
};
use lazy_static::lazy_static;
use prometheus::IntCounter;
use sea_orm::{ActiveModelTrait, ActiveValue::Set, DatabaseConnection, EntityTrait};
use uuid::Uuid;

lazy_static! {
    static ref PAYMENT_METHOD_UPDATES: IntCounter = IntCounter::new(
        "payment_method_updates_total",
        "Total number of payment method updates"
    )
    .expect("metric can be created");
    static ref PAYMENT_METHOD_UPDATE_FAILURES: IntCounter = IntCounter::new(
        "payment_method_update_failures_total",
        "Total number of failed payment method updates"
    )
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
    async fn execute(
        &self,
        db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        let db = db_pool.as_ref();

        let updated_order = self.update_payment_method(db).await?;

        self.log_and_trigger_event(event_sender, &updated_order)
            .await?;

        PAYMENT_METHOD_UPDATES.inc();
        info!(order_id = %self.order_id, "Payment method updated successfully");

        Ok(updated_order)
    }
}

impl UpdatePaymentMethodCommand {
    async fn update_payment_method(
        &self,
        db: &DatabaseConnection,
    ) -> Result<order_entity::Model, ServiceError> {
        let order = Order::find_by_id(self.order_id)
            .one(db)
            .await
            .map_err(|e| {
                error!("Failed to find order: {}", e);
                ServiceError::db_error(e)
            })?
            .ok_or_else(|| {
                let msg = format!("Order {} not found", self.order_id);
                error!("{}", msg);
                ServiceError::NotFound(msg)
            })?;

        let mut order_active_model: order_entity::ActiveModel = order.into();
        order_active_model.payment_method = Set(self.new_method.clone());
        order_active_model.updated_at = Set(chrono::Utc::now());

        order_active_model.update(db).await.map_err(|e| {
            error!("Failed to update payment method: {}", e);
            ServiceError::db_error(e)
        })
    }

    async fn log_and_trigger_event(
        &self,
        event_sender: Arc<EventSender>,
        _updated_order: &order_entity::Model,
    ) -> Result<(), ServiceError> {
        event_sender
            .send(Event::OrderUpdated {
                order_id: self.order_id,
                checkout_session_id: None,
                status: None,
                refunds: vec![],
            })
            .await
            .map_err(|e| {
                let msg = format!("Failed to send event for payment method update: {}", e);
                error!("{}", msg);
                ServiceError::EventError(msg)
            })?;
        Ok(())
    }
}
