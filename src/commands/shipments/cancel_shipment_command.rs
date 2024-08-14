use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use crate::{errors::ServiceError, db::DbPool, models::{Shipment, ShipmentStatus, NewShipmentNote}};
use crate::events::{Event, EventSender};
use tracing::{info, error, instrument};
use diesel::prelude::*;

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct CancelShipmentCommand {
    pub shipment_id: i32,

    #[validate(length(min = 1))]
    pub reason: String,
}

#[async_trait::async_trait]
impl Command for CancelShipmentCommand {
    type Result = Shipment;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|e| {
            error!("Failed to get database connection: {}", e);
            ServiceError::DatabaseError("Failed to get database connection".into())
        })?;

        let updated_shipment = conn.transaction::<Shipment, ServiceError, _>(|| {
            self.cancel_shipment(&conn)?;
            self.log_cancellation_reason(&conn)?;
            Ok(self.fetch_updated_shipment(&conn)?)
        }).map_err(|e| {
            error!("Transaction failed for cancelling shipment ID {}: {}", self.shipment_id, e);
            e
        })?;

        self.log_and_trigger_event(event_sender, &updated_shipment).await?;

        Ok(updated_shipment)
    }
}

impl CancelShipmentCommand {
    fn cancel_shipment(&self, conn: &PgConnection) -> Result<(), ServiceError> {
        diesel::update(shipments::table.find(self.shipment_id))
            .set(shipments::status.eq(ShipmentStatus::Cancelled))
            .execute(conn)
            .map_err(|e| {
                error!("Failed to cancel shipment ID {}: {}", self.shipment_id, e);
                ServiceError::DatabaseError(format!("Failed to cancel shipment: {}", e))
            })?;
        Ok(())
    }

    fn log_cancellation_reason(&self, conn: &PgConnection) -> Result<(), ServiceError> {
        diesel::insert_into(shipment_notes::table)
            .values(&NewShipmentNote { shipment_id: self.shipment_id, note: self.reason.clone() })
            .execute(conn)
            .map_err(|e| {
                error!("Failed to log cancellation reason for shipment ID {}: {}", self.shipment_id, e);
                ServiceError::DatabaseError(format!("Failed to log cancellation reason: {}", e))
            })?;
        Ok(())
    }

    fn fetch_updated_shipment(&self, conn: &PgConnection) -> Result<Shipment, ServiceError> {
        shipments::table.find(self.shipment_id)
            .get_result::<Shipment>(conn)
            .map_err(|e| {
                error!("Failed to fetch updated shipment for ID {}: {}", self.shipment_id, e);
                ServiceError::DatabaseError(format!("Failed to fetch updated shipment: {}", e))
            })
    }

    async fn log_and_trigger_event(&self, event_sender: Arc<EventSender>, updated_shipment: &Shipment) -> Result<(), ServiceError> {
        info!("Shipment cancelled for shipment ID: {}. Reason: {}", self.shipment_id, self.reason);
        event_sender.send(Event::ShipmentCancelled(self.shipment_id))
            .await
            .map_err(|e| {
                error!("Failed to send ShipmentCancelled event for shipment ID {}: {}", self.shipment_id, e);
                ServiceError::EventError(e.to_string())
            })
    }
}
