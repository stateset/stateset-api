use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use crate::{errors::ServiceError, db::DbPool, models::Shipment};
use crate::events::{Event, EventSender};
use tracing::{info, error, instrument};
use diesel::prelude::*;

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct AssignShipmentCarrierCommand {
    pub shipment_id: i32,
    
    #[validate(length(min = 1))]
    pub carrier_name: String, // Name of the shipping carrier
}

#[async_trait::async_trait]
impl Command for AssignShipmentCarrierCommand {
    type Result = Shipment;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|e| {
            error!("Failed to get database connection: {}", e);
            ServiceError::DatabaseError("Failed to get database connection".into())
        })?;

        let updated_shipment = conn.transaction(|| {
            self.assign_carrier(&conn)
        }).map_err(|e| {
            error!("Transaction failed for assigning carrier to shipment ID {}: {}", self.shipment_id, e);
            e
        })?;

        self.log_and_trigger_event(event_sender, &updated_shipment).await?;

        Ok(updated_shipment)
    }
}

impl AssignShipmentCarrierCommand {
    fn assign_carrier(&self, conn: &PgConnection) -> Result<Shipment, ServiceError> {
        diesel::update(shipments::table.find(self.shipment_id))
            .set(shipments::carrier.eq(self.carrier_name.clone()))
            .get_result::<Shipment>(conn)
            .map_err(|e| {
                error!("Failed to assign carrier to shipment ID {}: {}", self.shipment_id, e);
                ServiceError::DatabaseError(format!("Failed to assign carrier: {}", e))
            })
    }

    async fn log_and_trigger_event(&self, event_sender: Arc<EventSender>, shipment: &Shipment) -> Result<(), ServiceError> {
        info!("Carrier assigned to shipment ID: {}. Carrier: {}", self.shipment_id, self.carrier_name);
        event_sender.send(Event::CarrierAssignedToShipment(self.shipment_id, self.carrier_name.clone()))
            .await
            .map_err(|e| {
                error!("Failed to send CarrierAssignedToShipment event for shipment ID {}: {}", self.shipment_id, e);
                ServiceError::EventError(e.to_string())
            })
    }
}
