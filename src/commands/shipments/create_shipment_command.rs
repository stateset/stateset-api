use async_trait::async_trait;
use serde::{Serialize, Deserialize};
use std::sync::Arc;
use crate::{errors::ServiceError, db::DbPool, models::{Shipment, ShipmentStatus}};
use crate::events::{Event, EventSender};
use validator::Validate;
use tracing::info;
use chrono::{DateTime, Utc};


#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct CreateShipmentCommand {
    pub order_id: i32,
    pub shipping_address: String,

    #[validate(range(min = 1))]
    pub shipping_method: ShippingMethod, // Enum representing the shipping method
}

#[async_trait]
impl Command for CreateShipmentCommand {
    type Result = Shipment;

    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        // Validate the command
        self.validate().map_err(|e| ServiceError::ValidationError(e.to_string()))?;

        let conn = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;

        let shipment = Shipment {
            order_id: self.order_id,
            shipping_address: self.shipping_address.clone(),
            shipping_method: self.shipping_method.clone(),
            status: ShipmentStatus::Pending,
            // Additional fields like tracking number, timestamps, etc.
        };

        let saved_shipment = diesel::insert_into(shipments::table)
            .values(&shipment)
            .get_result::<Shipment>(&conn)
            .map_err(|e| ServiceError::DatabaseError)?;

        // Trigger an event
        event_sender.send(Event::ShipmentCreated(saved_shipment.id)).await.map_err(|e| ServiceError::EventError(e.to_string()))?;

        // Log the shipment creation
        info!("Shipment created: {:?}", saved_shipment);

        Ok(saved_shipment)
    }
}