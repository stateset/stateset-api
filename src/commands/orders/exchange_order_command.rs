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
    static ref ORDER_EXCHANGES: IntCounter = 
        IntCounter::new("order_exchanges_total", "Total number of order exchanges")
            .expect("metric can be created");

    static ref ORDER_EXCHANGE_FAILURES: IntCounter = 
        IntCounter::new("order_exchange_failures_total", "Total number of failed order exchanges")
            .expect("metric can be created");
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct ExchangeOrderCommand {
    #[validate(range(min = 1))]
    pub order_id: i32,

    #[validate(length(min = 1))]
    pub return_items: Vec<ReturnItem>,

    #[validate(length(min = 1))]
    pub new_items: Vec<NewOrderItem>,
}

#[async_trait]
impl Command for ExchangeOrderCommand {
    type Result = Order;

    #[instrument(skip(db_pool, event_sender))]
    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|e| {
            ORDER_EXCHANGE_FAILURES.inc();
            error!("Failed to get database connection: {}", e);
            ServiceError::DatabaseError
        })?;

        conn.transaction(|| {
            // Insert return items into return_items table
            for item in &self.return_items {
                diesel::insert_into(return_items::table)
                    .values(item)
                    .execute(&conn)
                    .map_err(|e| {
                        ORDER_EXCHANGE_FAILURES.inc();
                        error!("Failed to insert return item for order ID {}: {}", self.order_id, e);
                        ServiceError::DatabaseError
                    })?;
            }

            // Insert new items into order_items table
            for item in &self.new_items {
                diesel::insert_into(order_items::table)
                    .values(item)
                    .execute(&conn)
                    .map_err(|e| {
                        ORDER_EXCHANGE_FAILURES.inc();
                        error!("Failed to insert new order item for order ID {}: {}", self.order_id, e);
                        ServiceError::DatabaseError
                    })?;
            }

            // Update order status to Exchanged
            let updated_order = diesel::update(orders::table.find(self.order_id))
                .set(orders::status.eq(OrderStatus::Exchanged))
                .get_result::<Order>(&conn)
                .map_err(|e| {
                    ORDER_EXCHANGE_FAILURES.inc();
                    error!("Failed to update order status to Exchanged for order ID {}: {}", self.order_id, e);
                    ServiceError::DatabaseError
                })?;

            // Trigger an event
            if let Err(e) = event_sender.send(Event::OrderExchanged(self.order_id)).await {
                ORDER_EXCHANGE_FAILURES.inc();
                error!("Failed to send OrderExchanged event for order ID {}: {}", self.order_id, e);
                return Err(ServiceError::EventError(e.to_string()));
            }

            ORDER_EXCHANGES.inc();

            info!(
                order_id = %self.order_id,
                "Order exchanged successfully"
            );

            Ok(updated_order)
        })
    }
}
