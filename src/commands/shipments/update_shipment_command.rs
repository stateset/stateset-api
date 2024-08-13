use async_trait::async_trait;
use serde::{Serialize, Deserialize};
use std::sync::Arc;
use crate::{errors::ServiceError, db::DbPool, models::{Shipment, ShipmentStatus}};
use crate::events::{Event, EventSender};
use validator::Validate;
use tracing::info;
use chrono::{DateTime, Utc};

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct UpdateShipmentStatusCommand {
    pub shipment_id: i32,
    pub new_status: ShipmentStatus,
}

#[async_trait]
impl Command for UpdateShipmentStatusCommand {
    type Result = Shipment;

    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;

        // Update shipment status
        let updated_shipment = diesel::update(shipments::table.find(self.shipment_id))
            .set(shipments::status.eq(self.new_status.clone()))
            .get_result::<Shipment>(&conn)
            .map_err(|e| ServiceError::DatabaseError)?;

        // Log and trigger events
        info!("Shipment status updated for shipment ID: {}", self.shipment_id);
        event_sender.send(Event::ShipmentUpdated(self.shipment_id)).await.map_err(|e| ServiceError::EventError(e.to_string()))?;

        Ok(updated_shipment)
    }
}
