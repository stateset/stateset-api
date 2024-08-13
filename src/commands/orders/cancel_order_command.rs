use async_trait::async_trait;
use serde::{Serialize, Deserialize};
use std::sync::Arc;
use crate::{errors::OrderError, db::DbPool, models::{Order, OrderItem, OrderStatus, NewOrderNote}};
use crate::events::{Event, EventSender};
use validator::Validate;
use tracing::{info, error, instrument, warn};
use chrono::{DateTime, Utc};
use diesel::prelude::*;
use prometheus::{IntCounter, IntCounterVec};

lazy_static! {
    static ref ORDER_CANCELLATIONS: IntCounter = 
        IntCounter::new("order_cancellations_total", "Total number of order cancellations")
            .expect("metric can be created");

    static ref ORDER_CANCELLATION_FAILURES: IntCounterVec = 
        IntCounterVec::new(
            "order_cancellation_failures_total",
            "Total number of failed order cancellations",
            &["error_type"]
        ).expect("metric can be created");
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct CancelOrderCommand {
    pub order_id: i32,
    #[validate(length(min = 1, max = 500, message = "Reason must be between 1 and 500 characters"))]
    pub reason: String,
    pub version: i32,  // For optimistic locking
}

#[async_trait]
impl Command for CancelOrderCommand {
    type Result = Order;

    #[instrument(skip(db_pool, event_sender))]
    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, OrderError> {
        let conn = db_pool.get().map_err(|e| {
            ORDER_CANCELLATION_FAILURES.with_label_values(&["db_pool_error"]).inc();
            error!("Failed to get database connection: {}", e);
            OrderError::DatabaseError(e.into())
        })?;

        let updated_order = match cancel_order_in_db(&conn, self.order_id, &self.reason, self.version) {
            Ok(order) => order,
            Err(e) => {
                ORDER_CANCELLATION_FAILURES.with_label_values(&[e.error_type()]).inc();
                error!("Failed to cancel order: {}", e);
                return Err(e);
            }
        };

        if let Err(e) = event_sender.send(Event::OrderCancelled(self.order_id)).await {
            ORDER_CANCELLATION_FAILURES.with_label_values(&["event_error"]).inc();
            error!("Failed to send OrderCancelled event: {}", e);
            return Err(OrderError::EventError(e.to_string()));
        }

        ORDER_CANCELLATIONS.inc();

        info!(
            order_id = %self.order_id,
            reason = %self.reason,
            "Order canceled successfully"
        );

        Ok(updated_order)
    }
}

#[instrument(skip(conn))]
fn cancel_order_in_db(conn: &PgConnection, order_id: i32, reason: &str, version: i32) -> Result<Order, OrderError> {
    conn.transaction(|| {
        let updated_order = diesel::update(orders::table.find(order_id))
            .set((
                orders::status.eq(OrderStatus::Cancelled),
                orders::version.eq(orders::version + 1)
            ))
            .filter(orders::version.eq(version))
            .get_result::<Order>(conn)
            .map_err(|e| match e {
                diesel::result::Error::NotFound => OrderError::NotFound(order_id),
                _ => {
                    error!("Failed to update order status: {}", e);
                    OrderError::DatabaseError(e.into())
                }
            })?;

        if updated_order.version != version + 1 {
            warn!("Concurrent modification detected for order {}", order_id);
            return Err(OrderError::ConcurrentModification(order_id));
        }

        diesel::insert_into(order_notes::table)
            .values(&NewOrderNote { order_id, note: reason.to_string() })
            .execute(conn)
            .map_err(|e| {
                error!("Failed to insert order note: {}", e);
                OrderError::DatabaseError(e.into())
            })?;

        Ok(updated_order)
    })
}

// Extend the OrderError enum to include an error type
#[derive(thiserror::Error, Debug)]
pub enum OrderError {
    #[error("Order {0} not found")]
    NotFound(i32),
    #[error("Cannot cancel order {0} in current status")]
    InvalidStatus(i32),
    #[error("Database error: {0}")]
    DatabaseError(#[from] diesel::result::Error),
    #[error("Event error: {0}")]
    EventError(String),
    #[error("Concurrent modification of order {0}")]
    ConcurrentModification(i32),
}

impl OrderError {
    pub fn error_type(&self) -> &str {
        match self {
            OrderError::NotFound(_) => "not_found",
            OrderError::InvalidStatus(_) => "invalid_status",
            OrderError::DatabaseError(_) => "database_error",
            OrderError::EventError(_) => "event_error",
            OrderError::ConcurrentModification(_) => "concurrent_modification",
        }
    }
}
