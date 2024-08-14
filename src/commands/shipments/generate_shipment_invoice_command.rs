use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use crate::{errors::ServiceError, db::DbPool, models::{Shipment, Invoice}};
use crate::events::{Event, EventSender};
use tracing::{info, error, instrument};
use diesel::prelude::*;
use chrono::Utc;

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct GenerateShipmentInvoiceCommand {
    pub shipment_id: i32,
    pub total_amount: f64, // The total amount to be charged for the shipment
}

#[async_trait::async_trait]
impl Command for GenerateShipmentInvoiceCommand {
    type Result = Invoice;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|e| {
            error!("Failed to get database connection: {}", e);
            ServiceError::DatabaseError("Failed to get database connection".into())
        })?;

        let invoice = conn.transaction(|| {
            self.create_invoice(&conn)
        }).map_err(|e| {
            error!("Transaction failed for generating invoice for shipment ID {}: {}", self.shipment_id, e);
            e
        })?;

        self.log_and_trigger_event(event_sender, &invoice).await?;

        Ok(invoice)
    }
}

impl GenerateShipmentInvoiceCommand {
    fn create_invoice(&self, conn: &PgConnection) -> Result<Invoice, ServiceError> {
        let invoice = Invoice {
            shipment_id: self.shipment_id,
            amount: self.total_amount,
            created_at: Utc::now(),
            // Additional fields like taxes, breakdown of costs, etc.
        };

        diesel::insert_into(invoices::table)
            .values(&invoice)
            .get_result::<Invoice>(conn)
            .map_err(|e| {
                error!("Failed to create invoice for shipment ID {}: {}", self.shipment_id, e);
                ServiceError::DatabaseError(format!("Failed to create invoice: {}", e))
            })
    }

    async fn log_and_trigger_event(&self, event_sender: Arc<EventSender>, invoice: &Invoice) -> Result<(), ServiceError> {
        info!("Invoice generated for shipment ID: {}. Total amount: {}", self.shipment_id, self.total_amount);
        event_sender.send(Event::InvoiceGenerated(self.shipment_id, invoice.id))
            .await
            .map_err(|e| {
                error!("Failed to send InvoiceGenerated event for shipment ID {}: {}", self.shipment_id, e);
                ServiceError::EventError(e.to_string())
            })
    }
}
