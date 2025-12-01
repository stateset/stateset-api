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
    static ref BILLING_ADDRESS_UPDATES: IntCounter = IntCounter::new(
        "billing_address_updates_total",
        "Total number of billing address updates"
    )
    .expect("metric can be created");
    static ref BILLING_ADDRESS_UPDATE_FAILURES: IntCounter = IntCounter::new(
        "billing_address_update_failures_total",
        "Total number of failed billing address updates"
    )
    .expect("metric can be created");
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct UpdateBillingAddressCommand {
    pub order_id: Uuid,

    #[validate(length(min = 5, max = 255))]
    pub new_address: String,
}

#[async_trait]
impl Command for UpdateBillingAddressCommand {
    type Result = order_entity::Model;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(
        &self,
        db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        let db = db_pool.as_ref();

        let updated_order = self.update_billing_address(db).await?;

        self.log_and_trigger_event(event_sender, &updated_order)
            .await?;

        BILLING_ADDRESS_UPDATES.inc();
        info!(order_id = %self.order_id, "Billing address updated successfully");

        Ok(updated_order)
    }
}

impl UpdateBillingAddressCommand {
    async fn update_billing_address(
        &self,
        db: &DatabaseConnection,
    ) -> Result<order_entity::Model, ServiceError> {
        let order = Order::find_by_id(self.order_id)
            .one(db)
            .await
            .map_err(|e| {
                BILLING_ADDRESS_UPDATE_FAILURES.inc();
                let msg = format!("Failed to find order ID {}: {}", self.order_id, e);
                error!("{}", msg);
                ServiceError::db_error(e)
            })?
            .ok_or_else(|| {
                BILLING_ADDRESS_UPDATE_FAILURES.inc();
                let msg = format!("Order ID {} not found", self.order_id);
                error!("{}", msg);
                ServiceError::NotFound(msg)
            })?;

        let mut order_active_model: order_entity::ActiveModel = order.into();
        order_active_model.billing_address = Set(self.new_address.clone());
        order_active_model.updated_at = Set(chrono::Utc::now());

        order_active_model.update(db).await.map_err(|e| {
            BILLING_ADDRESS_UPDATE_FAILURES.inc();
            let msg = format!(
                "Failed to update billing address for order ID {}: {}",
                self.order_id, e
            );
            error!("{}", msg);
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
                BILLING_ADDRESS_UPDATE_FAILURES.inc();
                error!(
                    "Failed to send BillingAddressUpdated event for order ID {}: {}",
                    self.order_id, e
                );
                ServiceError::EventError(e)
            })
    }
}
