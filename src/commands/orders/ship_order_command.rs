use async_trait::async_trait;
use serde::{Serialize, Deserialize};
use std::sync::Arc;
use crate::{errors::ServiceError, db::DbPool, models::{Order, OrderStatus}};
use crate::events::{Event, EventSender};
use validator::Validate;
use tracing::{info, error, instrument};
use diesel::prelude::*;
use prometheus::IntCounter;

lazy_static! {
    static ref ORDERS_SHIPPED: IntCounter = 
        IntCounter::new("orders_shipped_total", "Total number of orders shipped")
            .expect("metric can be created");

    static ref ORDER_SHIP_FAILURES: IntCounter = 
        IntCounter::new("order_ship_failures_total", "Total number of failed order shipments")
            .expect("metric can be created");
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ShipOrderCommand {
    pub order_id: i32,
    pub user_id: i32,
}

#[async_trait]
impl Command for ShipOrderCommand {
    type Result = Order;

    #[instrument(skip(db_pool, event_sender))]
    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|e| {
            ORDER_SHIP_FAILURES.inc();
            error!("Failed to get database connection: {}", e);
            ServiceError::DatabaseError
        })?;

        // Update the order status to 'Shipped'
        let shipped_order = diesel::update(orders::table.find(self.order_id))
            .set(orders::status.eq(OrderStatus::Shipped))
            .get_result::<Order>(&conn)
            .map_err(|e| {
                ORDER_SHIP_FAILURES.inc();
                error!("Failed to update order status to 'Shipped' for order {}: {}", self.order_id, e);
                ServiceError::DatabaseError
            })?;

        // Trigger an event indicating the order has been shipped
        if let Err(e) = event_sender.send(Event::OrderShipped(self.order_id)).await {
            ORDER_SHIP_FAILURES.inc();
            error!("Failed to send OrderShipped event for order {}: {}", self.order_id, e);
            return Err(ServiceError::EventError(e.to_string()));
        }

        ORDERS_SHIPPED.inc();

        info!(
            order_id = %self.order_id,
            user_id = %self.user_id,
            "Order successfully shipped"
        );

        Ok(shipped_order)
    }
}