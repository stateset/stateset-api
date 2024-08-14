use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use crate::{errors::ServiceError, db::DbPool, models::{Shipment, ShipmentStatus}};
use crate::events::{Event, EventSender};
use tracing::{info, error, instrument};
use diesel::prelude::*;
use chrono::NaiveDateTime;

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct RescheduleShipmentCommand {
    pub shipment_id: i32,

    #[validate]
    pub new_scheduled_date: NaiveDateTime, // The new date and time for the shipment
}

#[async_trait::async_trait]
impl Command for RescheduleShipmentCommand {
    type Result = Shipment;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|e| {
            error!("Failed to get database connection: {}", e);
            ServiceError::DatabaseError("Failed to get database connection".into())
        })?;

        let updated_shipment = conn.transaction(|| {
            self.reschedule_shipment(&conn)
        }).map_err(|e| {
            error!("Transaction failed for rescheduling shipment ID {}: {}", self.shipment_id, e);
            e
        })?;

        self.log_and_trigger_event(event_sender, &updated_shipment).await?;

        Ok(updated_shipment)
    }
}

impl RescheduleShipmentCommand {
    fn reschedule_shipment(&self, conn: &PgConnection) -> Result<Shipment, ServiceError> {
        diesel::update(shipments::table.find(self.shipment_id))
            .set(shipments::scheduled_date.eq(self.new_scheduled_date))
            .get_result::<Shipment>(conn)
            .map_err(|e| {
                error!("Failed to reschedule shipment ID {}: {}", self.shipment_id, e);
                ServiceError::DatabaseError(format!("Failed to reschedule shipment: {}", e))
            })
    }

    async fn log_and_trigger_event(&self, event_sender: Arc<EventSender>, shipment: &Shipment) -> Result<(), ServiceError> {
        info!("Shipment ID: {} rescheduled to: {}", self.shipment_id, self.new_scheduled_date);
        event_sender.send(Event::ShipmentRescheduled(self.shipment_id, self.new_scheduled_date))
            .await
            .map_err(|e| {
                error!("Failed to send ShipmentRescheduled event for shipment ID {}: {}", self.shipment_id, e);
                ServiceError::EventError(e.to_string())
            })
    }
}
