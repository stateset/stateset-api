use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use crate::{errors::ServiceError, db::DbPool, models::Shipment};
use crate::events::{Event, EventSender};
use tracing::{info, error, instrument};
use diesel::prelude::*;

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct NotifyCustomerShipmentUpdateCommand {
    pub shipment_id: i32,
    pub notification_message: String, // The message to be sent to the customer
}

#[async_trait::async_trait]
impl Command for NotifyCustomerShipmentUpdateCommand {
    type Result = ();

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|e| {
            error!("Failed to get database connection: {}", e);
            ServiceError::DatabaseError("Failed to get database connection".into())
        })?;

        let shipment = self.get_shipment(&conn)?;

        self.send_notification(&shipment).await?;

        self.log_and_trigger_event(event_sender, &shipment).await?;

        Ok(())
    }
}

impl NotifyCustomerShipmentUpdateCommand {
    fn get_shipment(&self, conn: &PgConnection) -> Result<Shipment, ServiceError> {
        shipments::table.find(self.shipment_id)
            .first::<Shipment>(conn)
            .map_err(|e| {
                error!("Failed to find shipment with ID {}: {}", self.shipment_id, e);
                ServiceError::DatabaseError(format!("Failed to find shipment: {}", e))
            })
    }

    async fn send_notification(&self, shipment: &Shipment) -> Result<(), ServiceError> {
        // Simulate sending a notification to the customer
        // In real-world scenarios, this could involve sending an email, SMS, or push notification
        info!("Notification sent to customer for shipment ID {}: {}", shipment.id, self.notification_message);
        Ok(())
    }

    async fn log_and_trigger_event(&self, event_sender: Arc<EventSender>, shipment: &Shipment) -> Result<(), ServiceError> {
        event_sender.send(Event::CustomerNotifiedOfShipmentUpdate(self.shipment_id))
            .await
            .map_err(|e| {
                error!("Failed to send CustomerNotifiedOfShipmentUpdate event for shipment ID {}: {}", self.shipment_id, e);
                ServiceError::EventError(e.to_string())
            })
    }
}
