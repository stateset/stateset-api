use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use crate::{errors::ServiceError, db::DbPool, models::{Shipment, ShipmentException}};
use crate::events::{Event, EventSender};
use tracing::{info, error, instrument};
use diesel::prelude::*;

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct HandleShipmentExceptionCommand {
    pub shipment_id: i32,
    
    #[validate(length(min = 1))]
    pub exception_message: String, // Description of the exception
}

#[async_trait::async_trait]
impl Command for HandleShipmentExceptionCommand {
    type Result = ();

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|e| {
            error!("Failed to get database connection: {}", e);
            ServiceError::DatabaseError("Failed to get database connection".into())
        })?;

        self.log_shipment_exception(&conn)?;

        self.log_and_trigger_event(event_sender).await?;

        Ok(())
    }
}

impl HandleShipmentExceptionCommand {
    fn log_shipment_exception(&self, conn: &PgConnection) -> Result<(), ServiceError> {
        diesel::insert_into(shipment_exceptions::table)
            .values(&ShipmentException {
                shipment_id: self.shipment_id,
                exception_message: self.exception_message.clone(),
                created_at: Utc::now(),
            })
            .execute(conn)
            .map_err(|e| {
                error!("Failed to log exception for shipment ID {}: {}", self.shipment_id, e);
                ServiceError::DatabaseError(format!("Failed to log exception: {}", e))
            })?;
        Ok(())
    }

    async fn log_and_trigger_event(&self, event_sender: Arc<EventSender>) -> Result<(), ServiceError> {
        info!("Exception logged for shipment ID: {}. Exception: {}", self.shipment_id, self.exception_message);
        event_sender.send(Event::ShipmentExceptionLogged(self.shipment_id))
            .await
            .map_err(|e| {
                error!("Failed to send ShipmentExceptionLogged event for shipment ID {}: {}", self.shipment_id, e);
                ServiceError::EventError(e.to_string())
            })
    }
}
