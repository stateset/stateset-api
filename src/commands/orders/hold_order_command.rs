use async_trait::async_trait;
use serde::{Serialize, Deserialize};
use std::sync::Arc;
use crate::{errors::OrderError, db::DbPool, models::{Order, OrderItem, OrderStatus, NewOrderNote}};
use crate::events::{Event, EventSender};
use validator::Validate;
use tracing::{info, error, instrument};
use chrono::{DateTime, Utc};
use diesel::prelude::*;
use prometheus::IntCounter;

lazy_static! {
    static ref ORDER_HOLDS: IntCounter = 
        IntCounter::new("order_holds", "Number of orders put on hold")
            .expect("metric can be created");
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct HoldOrderCommand {
    pub order_id: i32,
    #[validate(length(min = 1, max = 500, message = "Reason must be between 1 and 500 characters"))]
    pub reason: String,
    pub version: i32,  // Added for optimistic locking
}

#[async_trait]
impl Command for HoldOrderCommand {
    type Result = Order;

    #[instrument(skip(db_pool, event_sender))]
    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, OrderError> {
        let conn = db_pool.get().map_err(|e| OrderError::DatabaseError(e.into()))?;

        let updated_order = hold_order_in_db(&conn, self.order_id, &self.reason, self.version)?;

        event_sender.send(Event::OrderOnHold(self.order_id))
            .await
            .map_err(|e| OrderError::EventError(e.to_string()))?;

        ORDER_HOLDS.inc();

        info!(
            order_id = %self.order_id,
            reason = %self.reason,
            "Order put on hold successfully"
        );

        Ok(updated_order)
    }
}

fn hold_order_in_db(conn: &PgConnection, order_id: i32, reason: &str, version: i32) -> Result<Order, OrderError> {
    conn.transaction(|| {
        let updated_order = diesel::update(orders::table.find(order_id))
            .set((
                orders::status.eq(OrderStatus::OnHold),
                orders::version.eq(orders::version + 1)
            ))
            .filter(orders::version.eq(version))
            .get_result::<Order>(conn)
            .map_err(|e| match e {
                diesel::result::Error::NotFound => OrderError::NotFound(order_id),
                _ => OrderError::DatabaseError(e.into())
            })?;

        if updated_order.version != version + 1 {
            return Err(OrderError::ConcurrentModification(order_id));
        }

        diesel::insert_into(order_notes::table)
            .values(&NewOrderNote { order_id, note: reason.to_string() })
            .execute(conn)
            .map_err(|e| OrderError::DatabaseError(e.into()))?;

        Ok(updated_order)
    })
}

// Assuming you have defined this error type
#[derive(thiserror::Error, Debug)]
pub enum OrderError {
    #[error("Order {0} not found")]
    NotFound(i32),
    #[error("Cannot put order {0} on hold in current status")]
    InvalidStatus(i32),
    #[error("Database error: {0}")]
    DatabaseError(#[from] diesel::result::Error),
    #[error("Event error: {0}")]
    EventError(String),
    #[error("Concurrent modification of order {0}")]
    ConcurrentModification(i32),
}