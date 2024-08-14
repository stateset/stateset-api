use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use crate::{errors::ServiceError, db::DbPool, models::{Shipment, ShipmentStatus}};
use crate::events::{Event, EventSender};
use tracing::{info, error, instrument};
use diesel::prelude::*;

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct ReleaseShipmentHoldCommand {
    pub shipment_id: i32,
}

#[async_trait::async_trait]
impl Command for ReleaseShipmentHoldCommand {
    type Result = Shipment;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|e| {
            error!("Failed to get database connection: {}", e);
            ServiceError::DatabaseError("Failed to get database connection".into())
        })?;

        let updated_shipment = conn.transaction(|| {
            self.release_shipment_hold(&conn)
        }).map_err(|e| {
            error!("Transaction failed for releasing hold on shipment ID {}: {}", self.shipment_id, e);
            e
        })?;

        self.log_and_trigger_event(event_sender, &updated_shipment).await?;

        Ok(updated_shipment)
    }
}

impl ReleaseShipmentHoldCommand {
    fn release_shipment_hold(&self, conn: &PgConnection) -> Result<Shipment, ServiceError> {
        diesel::update(shipments::table.find(self.shipment_id))
            .set(shipments::status.eq(ShipmentStatus::Pending)) // Assuming "Pending" is the next status after hold
            .get_result::<Shipment>(conn)
            .map_err(|e| {
                error!("Failed to release hold for shipment ID {}: {}", self.shipment_id, e);
                ServiceError::DatabaseError(format!("Failed to release hold: {}", e))
            })
    }

    async fn log_and_trigger_event(&self, event_sender: Arc<EventSender>, shipment: &Shipment) -> Result<(), ServiceError> {
        info!("Shipment ID: {} hold released.", self.shipment_id);
        event_sender.send(Event::ShipmentHoldReleased(self.shipment_id))
            .await
            .map_err(|e| {
                error!("Failed to send ShipmentHoldReleased event for shipment ID {}: {}", self.shipment_id, e);
                ServiceError::EventError(e.to_string())
            })
    }
}
