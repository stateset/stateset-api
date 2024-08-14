use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use crate::{errors::ServiceError, db::DbPool, models::Shipment};
use crate::events::{Event, EventSender};
use tracing::{info, error, instrument};
use diesel::prelude::*;

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct TrackShipmentCommand {
    pub shipment_id: i32,
}

#[async_trait::async_trait]
impl Command for TrackShipmentCommand {
    type Result = Shipment;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|e| {
            error!("Failed to get database connection: {}", e);
            ServiceError::DatabaseError("Failed to get database connection".into())
        })?;

        let shipment = self.get_shipment(&conn)?;

        let tracking_info = self.fetch_tracking_info(&shipment.tracking_number).await?;

        self.log_tracking_info(&tracking_info);
        self.log_and_trigger_event(event_sender, &shipment).await?;

        Ok(shipment)
    }
}

impl TrackShipmentCommand {
    fn get_shipment(&self, conn: &PgConnection) -> Result<Shipment, ServiceError> {
        shipments::table
            .find(self.shipment_id)
            .first::<Shipment>(conn)
            .map_err(|e| {
                error!("Failed to find shipment with ID {}: {}", self.shipment_id, e);
                ServiceError::DatabaseError(format!("Failed to find shipment: {}", e))
            })
    }

    async fn fetch_tracking_info(&self, tracking_number: &str) -> Result<String, ServiceError> {
        // Simulate fetching tracking info from an external API
        // This would normally involve making an HTTP request to the carrier's API
        Ok(format!("Tracking info for {}", tracking_number)) // Placeholder
    }

    fn log_tracking_info(&self, tracking_info: &str) {
        info!("Tracking info: {}", tracking_info);
    }

    async fn log_and_trigger_event(&self, event_sender: Arc<EventSender>, shipment: &Shipment) -> Result<(), ServiceError> {
        event_sender.send(Event::ShipmentTracked(self.shipment_id))
            .await
            .map_err(|e| {
                error!("Failed to send ShipmentTracked event for shipment ID {}: {}", self.shipment_id, e);
                ServiceError::EventError(e.to_string())
            })
    }
}
