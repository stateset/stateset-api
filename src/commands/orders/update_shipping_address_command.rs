use async_trait::async_trait;
use serde::{Serialize, Deserialize};
use std::sync::Arc;
use crate::{errors::ServiceError, db::DbPool, models::{Order, OrderStatus}};
use crate::events::{Event, EventSender};
use validator::Validate;
use tracing::{info, error, instrument};
use diesel::prelude::*;
use prometheus::IntCounter;

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

#[async_trait]
impl Command for UpdateShippingAddressCommand {
    type Result = Order;

    #[instrument(skip(db_pool, event_sender))]
    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|e| {
            SHIPPING_ADDRESS_UPDATE_FAILURES.inc();
            error!("Failed to get database connection: {}", e);
            ServiceError::DatabaseError
        })?;

        let updated_order = diesel::update(orders::table.find(self.order_id))
            .set(orders::shipping_address.eq(&self.new_address))
            .get_result::<Order>(&conn)
            .map_err(|e| {
                SHIPPING_ADDRESS_UPDATE_FAILURES.inc();
                error!("Failed to update shipping address for order ID {}: {}", self.order_id, e);
                ServiceError::DatabaseError
            })?;

        if let Err(e) = event_sender.send(Event::ShippingAddressUpdated(self.order_id)).await {
            SHIPPING_ADDRESS_UPDATE_FAILURES.inc();
            error!("Failed to send ShippingAddressUpdated event for order ID {}: {}", self.order_id, e);
            return Err(ServiceError::EventError(e.to_string()));
        }

        SHIPPING_ADDRESS_UPDATES.inc();

        info!(
            order_id = %self.order_id,
            "Shipping address updated successfully"
        );

        Ok(updated_order)
    }
}
