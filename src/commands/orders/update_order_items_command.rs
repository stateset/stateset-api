use async_trait::async_trait;
use serde::{Serialize, Deserialize};
use std::sync::Arc;
use crate::{errors::ServiceError, db::DbPool, models::{Order, OrderItem, OrderStatus}};
use crate::events::{Event, EventSender};
use validator::Validate;
use tracing::{info, error, instrument};
use diesel::prelude::*;
use prometheus::IntCounter;

lazy_static! {
    static ref ORDER_ITEM_UPDATES: IntCounter = 
        IntCounter::new("order_item_updates_total", "Total number of order item updates")
            .expect("metric can be created");

    static ref ORDER_ITEM_UPDATE_FAILURES: IntCounter = 
        IntCounter::new("order_item_update_failures_total", "Total number of failed order item updates")
            .expect("metric can be created");
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct UpdateOrderItemsCommand {
    pub order_id: i32,
    #[validate(length(min = 1))]
    pub items: Vec<OrderItem>,
}

#[async_trait]
impl Command for UpdateOrderItemsCommand {
    type Result = Order;

    #[instrument(skip(db_pool, event_sender))]
    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|e| {
            ORDER_ITEM_UPDATE_FAILURES.inc();
            error!("Failed to get database connection: {}", e);
            ServiceError::DatabaseError
        })?;

        // Begin transaction to ensure atomicity
        let updated_order = conn.transaction(|| {
            // Step 1: Remove existing items
            diesel::delete(order_items::table.filter(order_items::order_id.eq(self.order_id)))
                .execute(&conn)
                .map_err(|e| {
                    ORDER_ITEM_UPDATE_FAILURES.inc();
                    error!("Failed to delete order items for order ID {}: {}", self.order_id, e);
                    ServiceError::DatabaseError
                })?;

            // Step 2: Insert updated items
            for item in &self.items {
                diesel::insert_into(order_items::table)
                    .values(item)
                    .execute(&conn)
                    .map_err(|e| {
                        ORDER_ITEM_UPDATE_FAILURES.inc();
                        error!("Failed to insert order item for order ID {}: {}", self.order_id, e);
                        ServiceError::DatabaseError
                    })?;
            }

            // Step 3: Recalculate the order total
            let updated_order = recalculate_order_total(self.order_id, &conn)?;

            Ok(updated_order)
        }).map_err(|e: ServiceError| {
            error!("Transaction failed for updating order items in order ID {}: {}", self.order_id, e);
            e
        })?;

        // Log and trigger events
        if let Err(e) = event_sender.send(Event::OrderUpdated(self.order_id)).await {
            ORDER_ITEM_UPDATE_FAILURES.inc();
            error!("Failed to send OrderUpdated event for order ID {}: {}", self.order_id, e);
            return Err(ServiceError::EventError(e.to_string()));
        }

        ORDER_ITEM_UPDATES.inc();

        info!(
            order_id = %self.order_id,
            "Order items updated successfully"
        );

        Ok(updated_order)
    }
}
