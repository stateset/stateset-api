use async_trait::async_trait;
use serde::{Serialize, Deserialize};
use std::sync::Arc;
use crate::{errors::ServiceError, db::DbPool, models::{Order, OrderStatus, ReturnItem, NewOrderNote}};
use crate::events::{Event, EventSender};
use validator::Validate;
use tracing::{info, error, instrument};
use diesel::prelude::*;
use prometheus::IntCounter;

lazy_static! {
    static ref ORDER_RETURNS: IntCounter = 
        IntCounter::new("order_returns_total", "Total number of order returns")
            .expect("metric can be created");

    static ref ORDER_RETURN_FAILURES: IntCounter = 
        IntCounter::new("order_return_failures_total", "Total number of failed order returns")
            .expect("metric can be created");
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct ReturnOrderCommand {
    pub order_id: i32,
    #[validate(length(min = 1))]
    pub reason: String,
    #[validate(length(min = 1))]
    pub items: Vec<ReturnItem>,
}

#[async_trait]
impl Command for ReturnOrderCommand {
    type Result = Order;

    #[instrument(skip(db_pool, event_sender))]
    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|e| {
            ORDER_RETURN_FAILURES.inc();
            error!("Failed to get database connection: {}", e);
            ServiceError::DatabaseError
        })?;

        conn.transaction(|| {
            // Update order status to Returned
            let updated_order = diesel::update(orders::table.find(self.order_id))
                .set(orders::status.eq(OrderStatus::Returned))
                .get_result::<Order>(&conn)
                .map_err(|e| {
                    ORDER_RETURN_FAILURES.inc();
                    error!("Failed to update order status to Returned for order ID {}: {}", self.order_id, e);
                    ServiceError::DatabaseError
                })?;

            // Insert return items into return_items table
            for item in &self.items {
                diesel::insert_into(return_items::table)
                    .values(item)
                    .execute(&conn)
                    .map_err(|e| {
                        ORDER_RETURN_FAILURES.inc();
                        error!("Failed to insert return item for order ID {}: {}", self.order_id, e);
                        ServiceError::DatabaseError
                    })?;
            }

            // Log the return reason
            diesel::insert_into(order_notes::table)
                .values(&NewOrderNote { order_id: self.order_id, note: self.reason.clone() })
                .execute(&conn)
                .map_err(|e| {
                    ORDER_RETURN_FAILURES.inc();
                    error!("Failed to insert return note for order ID {}: {}", self.order_id, e);
                    ServiceError::DatabaseError
                })?;

            // Trigger an event
            if let Err(e) = event_sender.send(Event::OrderReturned(self.order_id)).await {
                ORDER_RETURN_FAILURES.inc();
                error!("Failed to send OrderReturned event for order ID {}: {}", self.order_id, e);
                return Err(ServiceError::EventError(e.to_string()));
            }

            ORDER_RETURNS.inc();

            info!(
                order_id = %self.order_id,
                "Order returned successfully"
            );

            Ok(updated_order)
        })
    }
}
