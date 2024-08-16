use async_trait::async_trait;;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info, instrument};
use validator::Validate;

use crate::{
    db::DbPool,
    errors::ServiceError,
    events::{Event, EventSender},
    models::{order, OrderStatus},
};
use sea_orm::{ActiveModelTrait, ActiveValue::Set, DatabaseConnection, EntityTrait};
use prometheus::IntCounter;
use lazy_static::lazy_static;

lazy_static! {
    static ref SHIPPING_ADDRESS_UPDATES: IntCounter = 
        IntCounter::new("shipping_address_updates_total", "Total number of shipping address updates")
            .expect("metric can be created");

    static ref SHIPPING_ADDRESS_UPDATE_FAILURES: IntCounter = 
        IntCounter::new("shipping_address_update_failures_total", "Total number of failed shipping address updates")
            .expect("metric can be created");
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct UpdateShippingAddressCommand {
    #[validate(range(min = 1))]
    pub order_id: i32,
    
    #[validate(length(min = 5, max = 255))]
    pub new_address: String,
}

#[async_trait::async_trait]
impl Command for UpdateShippingAddressCommand {
    type Result = order::Model;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        let db = db_pool.clone();

        let updated_order = self.update_shipping_address(&db).await?;

        self.log_and_trigger_event(event_sender, &updated_order).await?;

        SHIPPING_ADDRESS_UPDATES.inc();
        info!(order_id = %self.order_id, "Shipping address updated successfully");

        Ok(updated_order)
    }
}

impl UpdateShippingAddressCommand {
    async fn update_shipping_address(&self, db: &DatabaseConnection) -> Result<order::Model, ServiceError> {
        let order = order::Entity::find_by_id(self.order_id)
            .one(db)
            .await
            .map_err(|e| {
                SHIPPING_ADDRESS_UPDATE_FAILURES.inc();
                error!("Failed to find order ID {}: {}", self.order_id, e);
                ServiceError::DatabaseError
            })?
            .ok_or_else(|| {
                SHIPPING_ADDRESS_UPDATE_FAILURES.inc();
                error!("Order ID {} not found", self.order_id);
                ServiceError::NotFound
            })?;

        let mut order_active_model: order::ActiveModel = order.into();

        order_active_model.shipping_address = Set(self.new_address.clone());

        order_active_model
            .update(db)
            .await
            .map_err(|e| {
                SHIPPING_ADDRESS_UPDATE_FAILURES.inc();
                error!("Failed to update shipping address for order ID {}: {}", self.order_id, e);
                ServiceError::DatabaseError
            })
    }

    async fn log_and_trigger_event(&self, event_sender: Arc<EventSender>, updated_order: &order::Model) -> Result<(), ServiceError> {
        event_sender
            .send(Event::ShippingAddressUpdated(self.order_id))
            .await
            .map_err(|e| {
                SHIPPING_ADDRESS_UPDATE_FAILURES.inc();
                error!("Failed to send ShippingAddressUpdated event for order ID {}: {}", self.order_id, e);
                ServiceError::EventError(e.to_string())
            })
    }
}
