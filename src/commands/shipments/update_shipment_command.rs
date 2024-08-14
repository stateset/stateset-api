use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use crate::{errors::ServiceError, db::DbPool, models::{Shipment, ShipmentStatus}};
use crate::events::{Event, EventSender};
use validator::Validate;
use tracing::{info, error, instrument};
use diesel::prelude::*;

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct UpdateShipmentStatusCommand {
    pub shipment_id: i32,
    
    #[validate]
    pub new_status: ShipmentStatus,
}

#[async_trait::async_trait]
impl Command for UpdateShipmentStatusCommand {
    type Result = Shipment;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|e| {
            error!("Failed to get database connection: {}", e);
            ServiceError::DatabaseError("Failed to get database connection".into())
        })?;

        let updated_shipment = conn.transaction(|| {
            self.update_shipment_status(&conn)
        }).map_err(|e| {
            error!("Transaction failed for updating shipment status for shipment ID {}: {}", self.shipment_id, e);
            e
        })?;

        self.log_and_trigger_event(event_sender, &updated_shipment).await?;

        Ok(updated_shipment)
    }
}

impl UpdateShipmentStatusCommand {
    fn update_shipment_status(&self, conn: &PgConnection) -> Result<Shipment, ServiceError> {
        diesel::update(shipments::table.find(self.shipment_id))
            .set(shipments::status.eq(self.new_status.clone()))
            .get_result::<Shipment>(conn)
            .map_err(|e| {
                error!("Failed to update shipment status for shipment ID {}: {}", self.shipment_id, e);
                ServiceError::DatabaseError(format!("Failed to update shipment status: {}", e))
            })
    }

    async fn log_and_trigger_event(&self, event_sender: Arc<EventSender>, updated_shipment: &Shipment) -> Result<(), ServiceError> {
        info!("Shipment status updated for shipment ID: {}", self.shipment_id);
        event_sender.send(Event::ShipmentUpdated(self.shipment_id))
            .await
            .map_err(|e| {
                error!("Failed to send ShipmentUpdated event for shipment ID {}: {}", self.shipment_id, e);
                ServiceError::EventError(e.to_string())
            })
    }
}
