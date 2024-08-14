use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use crate::{errors::ServiceError, db::DbPool, models::Shipment};
use crate::events::{Event, EventSender};
use tracing::{info, error, instrument};
use diesel::prelude::*;

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct VerifyShipmentAddressCommand {
    pub shipment_id: i32,
}

#[async_trait::async_trait]
impl Command for VerifyShipmentAddressCommand {
    type Result = bool; // Returns true if the address is valid, false otherwise

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|e| {
            error!("Failed to get database connection: {}", e);
            ServiceError::DatabaseError("Failed to get database connection".into())
        })?;

        let shipment = self.get_shipment(&conn)?;

        let is_valid = self.verify_address(&shipment).await?;

        self.log_verification_result(is_valid);
        self.log_and_trigger_event(event_sender, &shipment, is_valid).await?;

        Ok(is_valid)
    }
}

impl VerifyShipmentAddressCommand {
    fn get_shipment(&self, conn: &PgConnection) -> Result<Shipment, ServiceError> {
        shipments::table.find(self.shipment_id)
            .first::<Shipment>(conn)
            .map_err(|e| {
                error!("Failed to find shipment with ID {}: {}", self.shipment_id, e);
                ServiceError::DatabaseError(format!("Failed to find shipment: {}", e))
            })
    }

    async fn verify_address(&self, shipment: &Shipment) -> Result<bool, ServiceError> {
        // Simulate address verification
        // In real-world scenarios, this could involve making an HTTP request to an address verification service
        let is_valid = true; // Placeholder for actual verification logic
        Ok(is_valid)
    }

    fn log_verification_result(&self, is_valid: bool) {
        info!("Address verification result: {}", if is_valid { "Valid" } else { "Invalid" });
    }

    async fn log_and_trigger_event(&self, event_sender: Arc<EventSender>, shipment: &Shipment, is_valid: bool) -> Result<(), ServiceError> {
        let event = if is_valid {
            Event::AddressVerified(self.shipment_id)
        } else {
            Event::AddressVerificationFailed(self.shipment_id)
        };

        event_sender.send(event)
            .await
            .map_err(|e| {
                error!("Failed to send address verification event for shipment ID {}: {}", self.shipment_id, e);
                ServiceError::EventError(e.to_string())
            })
    }
}
