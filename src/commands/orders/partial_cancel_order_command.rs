use async_trait::async_trait;
use serde::{Serialize, Deserialize};
use std::sync::Arc;
use crate::{errors::ServiceError, db::DbPool, models::{Order, OrderItem, OrderStatus}};
use crate::events::{Event, EventSender};
use validator::Validate;
use tracing::info;
use chrono::{DateTime, Utc};


#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct PartialCancelOrderCommand {
    pub order_id: i32,
    #[validate(length(min = 1))]
    pub item_ids: Vec<i32>, // IDs of items to cancel
}

#[async_trait]
impl Command for PartialCancelOrderCommand {
    type Result = Order;

    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;

        // Begin transaction to ensure atomicity
        conn.transaction(|| {
            // Step 1: Remove the specified items from the order
            diesel::delete(order_items::table.filter(order_items::id.eq_any(&self.item_ids)))
                .execute(&conn)
                .map_err(|e| ServiceError::DatabaseError)?;

            // Step 2: Recalculate the order total
            let updated_order = recalculate_order_total(self.order_id, &conn)?;

            // Step 3: Log and trigger events
            info!("Partial cancellation of items for order ID: {}", self.order_id);
            event_sender.send(Event::OrderUpdated(self.order_id)).await.map_err(|e| ServiceError::EventError(e.to_string()))?;

            Ok(updated_order)
        })
    }
}
