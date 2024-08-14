use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use crate::{errors::ServiceError, db::DbPool, models::{Shipment, ShipmentStatus}};
use crate::events::{Event, EventSender};
use tracing::{info, error, instrument};
use diesel::prelude::*;
use chrono::Utc;

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct ConfirmShipmentDeliveryCommand {
    pub shipment_id: i32,
}

#[async_trait::async_trait]
impl Command for ConfirmShipmentDeliveryCommand {
    type Result = Shipment;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|e| {
            error!("Failed to get database connection: {}", e);
            ServiceError::DatabaseError("Failed to get database connection".into())
        })?;

        let updated_shipment = conn.transaction(|| {
            self.confirm_delivery(&conn)
        }).map_err(|e| {
            error!("Transaction failed for confirming delivery of shipment ID {}: {}", self.shipment_id, e);
            e
        })?;

        self.log_and_trigger_event(event_sender, &updated_shipment).await?;

        Ok(updated_shipment)
    }
}

impl ConfirmShipmentDeliveryCommand {
    fn confirm_delivery(&self, conn: &PgConnection) -> Result<Shipment, ServiceError> {
        diesel::update(shipments::table.find(self.shipment_id))
            .set((
                shipments::status.eq(ShipmentStatus::Delivered),
                shipments::delivered_at.eq(Utc::now()),
            ))
            .get_result::<Shipment>(conn)
            .map_err(|e| {
                error!("Failed to confirm delivery for shipment ID {}: {}", self.shipment_id, e);
                ServiceError::DatabaseError(format!("Failed to confirm delivery: {}", e))
            })
    }

    async fn log_and_trigger_event(&self, event_sender: Arc<EventSender>, shipment: &Shipment) -> Result<(), ServiceError> {
        info!("Shipment ID: {} confirmed as delivered.", self.shipment_id);
        event_sender.send(Event::ShipmentDelivered(self.shipment_id))
            .await
            .map_err(|e| {
                error!("Failed to send ShipmentDelivered event for shipment ID {}: {}", self.shipment_id, e);
                ServiceError::EventError(e.to_string())
            })
    }
}
