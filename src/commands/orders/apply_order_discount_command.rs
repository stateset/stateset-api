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
    static ref ORDER_DISCOUNTS_APPLIED: IntCounter = 
        IntCounter::new("order_discounts_applied_total", "Total number of discounts applied to orders")
            .expect("metric can be created");

    static ref ORDER_DISCOUNT_FAILURES: IntCounter = 
        IntCounter::new("order_discount_failures_total", "Total number of failed order discount applications")
            .expect("metric can be created");
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct ApplyOrderDiscountCommand {
    #[validate(range(min = 1))]
    pub order_id: i32,

    #[validate(range(min = 0.01, message = "Discount amount must be greater than zero"))]
    pub discount_amount: f64,
}

#[async_trait]
impl Command for ApplyOrderDiscountCommand {
    type Result = Order;

    #[instrument(skip(db_pool, event_sender))]
    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|e| {
            ORDER_DISCOUNT_FAILURES.inc();
            error!("Failed to get database connection: {}", e);
            ServiceError::DatabaseError
        })?;

        conn.transaction(|| {
            // Apply discount to the order's total amount
            let updated_order = diesel::update(orders::table.find(self.order_id))
                .set(orders::total_amount.eq(orders::total_amount - self.discount_amount))
                .get_result::<Order>(&conn)
                .map_err(|e| {
                    ORDER_DISCOUNT_FAILURES.inc();
                    error!("Failed to apply discount to order ID {}: {}", self.order_id, e);
                    ServiceError::DatabaseError
                })?;

            // Send the OrderUpdated event
            if let Err(e) = event_sender.send(Event::OrderUpdated(self.order_id)).await {
                ORDER_DISCOUNT_FAILURES.inc();
                error!("Failed to send OrderUpdated event for order ID {}: {}", self.order_id, e);
                return Err(ServiceError::EventError(e.to_string()));
            }

            ORDER_DISCOUNTS_APPLIED.inc();

            info!(
                order_id = %self.order_id,
                discount_amount = %self.discount_amount,
                "Successfully applied discount to order"
            );

            Ok(updated_order)
        })
    }
}
