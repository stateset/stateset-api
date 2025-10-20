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
    static ref SHIPPING_METHOD_UPDATES: IntCounter = IntCounter::new(
        "shipping_method_updates_total",
        "Total number of shipping method updates"
    )
    .expect("metric can be created");
    static ref SHIPPING_METHOD_UPDATE_FAILURES: IntCounter = IntCounter::new(
        "shipping_method_update_failures_total",
        "Total number of failed shipping method updates"
    )
    .expect("metric can be created");
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct UpdateShippingMethodCommand {
    pub order_id: Uuid,

    #[validate(length(min = 1, max = 100))]
    pub new_method: String,
}

#[async_trait]
impl Command for UpdateShippingMethodCommand {
    type Result = order_entity::Model;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(
        &self,
        db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        let db = db_pool.as_ref();

        let updated_order = self.update_shipping_method(db).await?;

        self.log_and_trigger_event(event_sender, &updated_order)
            .await?;

        SHIPPING_METHOD_UPDATES.inc();
        info!(order_id = %self.order_id, "Shipping method updated successfully");

        Ok(updated_order)
    }
}

impl UpdateShippingMethodCommand {
    async fn update_shipping_method(
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
        order_active_model.shipping_method = Set(self.new_method.clone());
        order_active_model.updated_at = Set(chrono::Utc::now());

        order_active_model.update(db).await.map_err(|e| {
            error!("Failed to update shipping method: {}", e);
            ServiceError::db_error(e)
        })
    }

    async fn log_and_trigger_event(
        &self,
        event_sender: Arc<EventSender>,
        _updated_order: &order_entity::Model,
    ) -> Result<(), ServiceError> {
        event_sender
            .send(Event::ShippingMethodUpdated(self.order_id))
            .await
            .map_err(|e| {
                SHIPPING_METHOD_UPDATE_FAILURES.inc();
                error!(
                    "Failed to send ShippingMethodUpdated event for order ID {}: {}",
                    self.order_id, e
                );
                ServiceError::EventError(e)
            })
    }
}
