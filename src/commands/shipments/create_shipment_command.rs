use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use crate::{errors::ServiceError, db::DbPool, models::{Shipment, ShipmentStatus, ShippingMethod}};
use crate::events::{Event, EventSender};
use validator::Validate;
use tracing::{info, error, instrument};
use diesel::prelude::*;

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct CreateShipmentCommand {
    pub order_id: i32,
    
    #[validate(length(min = 1))]
    pub shipping_address: String,

    #[validate(range(min = 1))]
    pub shipping_method: ShippingMethod, // Enum representing the shipping method
}

#[async_trait::async_trait]
impl Command for CreateShipmentCommand {
    type Result = Shipment;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        self.validate().map_err(|e| {
            error!("Validation failed: {:?}", e);
            ServiceError::ValidationError(e.to_string())
        })?;

        let conn = db_pool.get().map_err(|e| {
            error!("Failed to get database connection: {}", e);
            ServiceError::DatabaseError("Failed to get database connection".into())
        })?;

        let saved_shipment = conn.transaction(|| {
            self.create_shipment(&conn)
        }).map_err(|e| {
            error!("Transaction failed for creating shipment: {}", e);
            e
        })?;

        self.log_and_trigger_event(event_sender, &saved_shipment).await?;

        Ok(saved_shipment)
    }
}

impl CreateShipmentCommand {
    fn create_shipment(&self, conn: &PgConnection) -> Result<Shipment, ServiceError> {
        let shipment = Shipment {
            order_id: self.order_id,
            shipping_address: self.shipping_address.clone(),
            shipping_method: self.shipping_method.clone(),
            status: ShipmentStatus::Pending,
            // Additional fields like tracking number, timestamps, etc.
        };

        diesel::insert_into(shipments::table)
            .values(&shipment)
            .get_result::<Shipment>(conn)
            .map_err(|e| {
                error!("Failed to create shipment: {}", e);
                ServiceError::DatabaseError(format!("Failed to create shipment: {}", e))
            })
    }

    async fn log_and_trigger_event(&self, event_sender: Arc<EventSender>, shipment: &Shipment) -> Result<(), ServiceError> {
        info!("Shipment created: {:?}", shipment);
        event_sender.send(Event::ShipmentCreated(shipment.id))
            .await
            .map_err(|e| {
                error!("Failed to send ShipmentCreated event for shipment ID {}: {}", shipment.id, e);
                ServiceError::EventError(e.to_string())
            })
    }
}
