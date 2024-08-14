use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use crate::{errors::ServiceError, db::DbPool, models::{PurchaseOrder, NewPurchaseOrder, PurchaseOrderItem, NewPurchaseOrderItem}};
use crate::events::{Event, EventSender};
use tracing::{info, error, instrument};
use diesel::prelude::*;
use chrono::Utc;

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct CreatePurchaseOrderCommand {
    pub supplier_id: i32,
    #[validate(length(min = 1))]
    pub items: Vec<PurchaseOrderItemData>, // List of items to be included in the purchase order
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct PurchaseOrderItemData {
    pub product_id: i32,
    #[validate(range(min = 1))]
    pub quantity: i32,
    #[validate(range(min = 0.0))]
    pub unit_price: f64,
}

#[async_trait::async_trait]
impl Command for CreatePurchaseOrderCommand {
    type Result = PurchaseOrder;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|e| {
            error!("Failed to get database connection: {}", e);
            ServiceError::DatabaseError("Failed to get database connection".into())
        })?;

        let purchase_order = conn.transaction(|| {
            let po = self.create_purchase_order(&conn)?;
            self.create_purchase_order_items(&conn, po.id)?;
            Ok(po)
        }).map_err(|e| {
            error!("Transaction failed for creating Purchase Order: {}", e);
            e
        })?;

        self.log_and_trigger_event(event_sender, &purchase_order).await?;

        Ok(purchase_order)
    }
}

impl CreatePurchaseOrderCommand {
    fn create_purchase_order(&self, conn: &PgConnection) -> Result<PurchaseOrder, ServiceError> {
        let new_po = NewPurchaseOrder {
            supplier_id: self.supplier_id,
            created_at: Utc::now(),
            status: "Pending".to_string(), // Assuming initial status is "Pending"
        };

        diesel::insert_into(purchase_orders::table)
            .values(&new_po)
            .get_result::<PurchaseOrder>(conn)
            .map_err(|e| {
                error!("Failed to create Purchase Order: {}", e);
                ServiceError::DatabaseError(format!("Failed to create Purchase Order: {}", e))
            })
    }

    fn create_purchase_order_items(&self, conn: &PgConnection, po_id: i32) -> Result<(), ServiceError> {
        for item in &self.items {
            let new_po_item = NewPurchaseOrderItem {
                purchase_order_id: po_id,
                product_id: item.product_id,
                quantity: item.quantity,
                unit_price: item.unit_price,
            };

            diesel::insert_into(purchase_order_items::table)
                .values(&new_po_item)
                .execute(conn)
                .map_err(|e| {
                    error!("Failed to create Purchase Order Item: {}", e);
                    ServiceError::DatabaseError(format!("Failed to create Purchase Order Item: {}", e))
                })?;
        }
        Ok(())
    }

    async fn log_and_trigger_event(&self, event_sender: Arc<EventSender>, purchase_order: &PurchaseOrder) -> Result<(), ServiceError> {
        info!("Purchase Order created with ID: {}", purchase_order.id);
        event_sender.send(Event::PurchaseOrderCreated(purchase_order.id))
            .await
            .map_err(|e| {
                error!("Failed to send PurchaseOrderCreated event for Purchase Order ID {}: {}", purchase_order.id, e);
                ServiceError::EventError(e.to_string())
            })
    }
}
