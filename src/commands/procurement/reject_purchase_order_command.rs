use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use crate::{errors::ServiceError, db::DbPool, models::PurchaseOrder};
use crate::events::{Event, EventSender};
use tracing::{info, error, instrument};
use diesel::prelude::*;

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct RejectPurchaseOrderCommand {
    pub purchase_order_id: i32,
    #[validate(length(min = 1))]
    pub reason: String, // Reason for rejecting the purchase order
}

#[async_trait::async_trait]
impl Command for RejectPurchaseOrderCommand {
    type Result = PurchaseOrder;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|e| {
            error!("Failed to get database connection: {}", e);
            ServiceError::DatabaseError("Failed to get database connection".into())
        })?;

        let updated_po = conn.transaction(|| {
            self.reject_purchase_order(&conn)
        }).map_err(|e| {
            error!("Transaction failed for rejecting Purchase Order ID {}: {}", self.purchase_order_id, e);
            e
        })?;

        self.log_and_trigger_event(event_sender, &updated_po).await?;

        Ok(updated_po)
    }
}

impl RejectPurchaseOrderCommand {
    fn reject_purchase_order(&self, conn: &PgConnection) -> Result<PurchaseOrder, ServiceError> {
        diesel::update(purchase_orders::table.find(self.purchase_order_id))
            .set((
                purchase_orders::status.eq("Rejected"),
                purchase_orders::rejection_reason.eq(self.reason.clone()),
            ))
            .get_result::<PurchaseOrder>(conn)
            .map_err(|e| {
                error!("Failed to reject Purchase Order ID {}: {}", self.purchase_order_id, e);
                ServiceError::DatabaseError(format!("Failed to reject Purchase Order: {}", e))
            })
    }

    async fn log_and_trigger_event(&self, event_sender: Arc<EventSender>, purchase_order: &PurchaseOrder) -> Result<(), ServiceError> {
        info!("Purchase Order ID: {} has been rejected. Reason: {}", self.purchase_order_id, self.reason);
        event_sender.send(Event::PurchaseOrderRejected(purchase_order.id))
            .await
            .map_err(|e| {
                error!("Failed to send PurchaseOrderRejected event for Purchase Order ID {}: {}", purchase_order.id, e);
                ServiceError::EventError(e.to_string())
            })
    }
}
