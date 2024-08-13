use async_trait::async_trait;
use serde::{Serialize, Deserialize};
use std::sync::Arc;
use crate::{errors::ServiceError, db::DbPool, models::{Shipment, ShipmentStatus}};
use crate::events::{Event, EventSender};
use validator::Validate;
use tracing::info;
use chrono::{DateTime, Utc};

pub struct CancelShipmentCommand {
    pub shipment_id: i32,
    pub reason: String,
}

#[async_trait]
impl Command for CancelShipmentCommand {
    type Result = Shipment;

    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;

        let updated_shipment = diesel::update(shipments::table.find(self.shipment_id))
            .set(shipments::status.eq(ShipmentStatus::Cancelled))
            .get_result::<Shipment>(&conn)
            .map_err(|e| ServiceError::DatabaseError)?;

        // Log the cancellation reason
        diesel::insert_into(shipment_notes::table)
            .values(&NewShipmentNote { shipment_id: self.shipment_id, note: self.reason.clone() })
            .execute(&conn)
            .map_err(|e| ServiceError::DatabaseError)?;

        event_sender.send(Event::ShipmentCancelled(self.shipment_id)).await.map_err(|e| ServiceError::EventError(e.to_string()))?;

        Ok(updated_shipment)
    }
}
