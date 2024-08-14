use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use crate::{errors::ServiceError, db::DbPool, models::Shipment};
use crate::events::{Event, EventSender};
use tracing::{info, error, instrument};
use diesel::prelude::*;

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct GenerateShippingLabelCommand {
    pub shipment_id: i32,
    pub label_url: String, // URL to the generated shipping label
}

#[async_trait::async_trait]
impl Command for GenerateShippingLabelCommand {
    type Result = Shipment;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|e| {
            error!("Failed to get database connection: {}", e);
            ServiceError::DatabaseError("Failed to get database connection".into())
        })?;

        let updated_shipment = conn.transaction(|| {
            self.attach_shipping_label(&conn)
        }).map_err(|e| {
            error!("Transaction failed for generating shipping label for shipment ID {}: {}", self.shipment_id, e);
            e
        })?;

        self.log_and_trigger_event(event_sender, &updated_shipment).await?;

        Ok(updated_shipment)
    }
}

impl GenerateShippingLabelCommand {
    fn attach_shipping_label(&self, conn: &PgConnection) -> Result<Shipment, ServiceError> {
        diesel::update(shipments::table.find(self.shipment_id))
            .set(shipments::label_url.eq(self.label_url.clone()))
            .get_result::<Shipment>(conn)
            .map_err(|e| {
                error!("Failed to attach shipping label for shipment ID {}: {}", self.shipment_id, e);
                ServiceError::DatabaseError(format!("Failed to attach shipping label: {}", e))
            })
    }

    async fn log_and_trigger_event(&self, event_sender: Arc<EventSender>, shipment: &Shipment) -> Result<(), ServiceError> {
        info!("Shipping label generated for shipment ID: {}.", self.shipment_id);
        event_sender.send(Event::ShippingLabelGenerated(self.shipment_id))
            .await
            .map_err(|e| {
                error!("Failed to send ShippingLabelGenerated event for shipment ID {}: {}", self.shipment_id, e);
                ServiceError::EventError(e.to_string())
            })
    }
}
