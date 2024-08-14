use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use crate::{errors::ServiceError, db::DbPool, models::{PurchaseOrder, InventoryItem}};
use crate::events::{Event, EventSender};
use tracing::{info, error, instrument};
use diesel::prelude::*;
use chrono::Utc;

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct ReceivePurchaseOrderCommand {
    pub purchase_order_id: i32,
    pub received_items: Vec<ReceivedItem>, // List of received items with quantities
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct ReceivedItem {
    pub product_id: i32,
    #[validate(range(min = 1))]
    pub quantity: i32,
}

#[async_trait::async_trait]
impl Command for ReceivePurchaseOrderCommand {
    type Result = PurchaseOrder;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|e| {
            error!("Failed to get database connection: {}", e);
            ServiceError::DatabaseError("Failed to get database connection".into())
        })?;

        let purchase_order = conn.transaction(|| {
            let po = self.mark_purchase_order_received(&conn)?;
            self.update_inventory(&conn)?;
            Ok(po)
        }).map_err(|e| {
            error!("Transaction failed for receiving Purchase Order ID {}: {}", self.purchase_order_id, e);
            e
        })?;

        self.log_and_trigger_event(event_sender, &purchase_order).await?;

        Ok(purchase_order)
    }
}

impl ReceivePurchaseOrderCommand {
    fn mark_purchase_order_received(&self, conn: &PgConnection) -> Result<PurchaseOrder, ServiceError> {
        diesel::update(purchase_orders::table.find(self.purchase_order_id))
            .set(purchase_orders::status.eq("Received"))
            .get_result::<PurchaseOrder>(conn)
            .map_err(|e| {
                error!("Failed to mark Purchase Order ID {} as received: {}", self.purchase_order_id, e);
                ServiceError::DatabaseError(format!("Failed to mark Purchase Order as received: {}", e))
            })
    }

    fn update_inventory(&self, conn: &PgConnection) -> Result<(), ServiceError> {
        for item in &self.received_items {
            diesel::insert_into(inventory_items::table)
                .values((
                    inventory_items::product_id.eq(item.product_id),
                    inventory_items::quantity.eq(item.quantity),
                    inventory_items::updated_at.eq(Utc::now()),
                ))
                .on_conflict(inventory_items::product_id)
                .do_update()
                .set(inventory_items::quantity.eq(inventory_items::quantity + item.quantity))
                .execute(conn)
                .map_err(|e| {
                    error!("Failed to update inventory for Product ID {}: {}", item.product_id, e);
                    ServiceError::DatabaseError(format!("Failed to update inventory: {}", e))
                })?;
        }
        Ok(())
    }

    async fn log_and_trigger_event(&self, event_sender: Arc<EventSender>, purchase_order: &PurchaseOrder) -> Result<(), ServiceError> {
        info!("Purchase Order ID: {} marked as received.", self.purchase_order_id);
        event_sender.send(Event::PurchaseOrderReceived(purchase_order.id))
            .await
            .map_err(|e| {
                error!("Failed to send PurchaseOrderReceived event for Purchase Order ID {}: {}", purchase_order.id, e);
                ServiceError::EventError(e.to_string())
            })
    }
}
