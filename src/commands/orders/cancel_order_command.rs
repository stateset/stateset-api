use async_trait::async_trait;;
use serde::{Serialize, Deserialize};
use std::sync::Arc;
use sea_orm::*;
use crate::{errors::OrderError, db::DbPool, models::{order_entity, order_entity::Entity as Order, order_note_entity, OrderStatus}};
use crate::events::{Event, EventSender};
use validator::Validate;
use tracing::{info, error, instrument, warn};
use chrono::Utc;
use prometheus::{IntCounter, IntCounterVec};
use lazy_static::lazy_static

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
    type Result = order_entity::Model;

    #[instrument(skip(db_pool, event_sender))]
    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, OrderError> {
        let db = db_pool.get().map_err(|e| {
            ORDER_CANCELLATION_FAILURES.with_label_values(&["db_pool_error"]).inc();
            error!("Failed to get database connection: {}", e);
            OrderError::DatabaseError(e.to_string())
        })?;

        let updated_order = match cancel_order_in_db(&db, self.order_id, &self.reason, self.version).await {
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

#[instrument(skip(db))]
async fn cancel_order_in_db(db: &DatabaseConnection, order_id: i32, reason: &str, version: i32) -> Result<order_entity::Model, OrderError> {
    let transaction_result = db.transaction::<_, order_entity::Model, OrderError>(|txn| {
        Box::pin(async move {
            let order = Order::find_by_id(order_id)
                .one(txn)
                .await
                .map_err(|e| OrderError::DatabaseError(e.to_string()))?
                .ok_or(OrderError::NotFound(order_id))?;

            if order.version != version {
                warn!("Concurrent modification detected for order {}", order_id);
                return Err(OrderError::ConcurrentModification(order_id));
            }

            let mut order: order_entity::ActiveModel = order.into();
            order.status = Set(OrderStatus::Cancelled.to_string());
            order.version = Set(version + 1);

            let updated_order = order.update(txn).await
                .map_err(|e| OrderError::DatabaseError(e.to_string()))?;

            let new_note = order_note_entity::ActiveModel {
                order_id: Set(order_id),
                note: Set(reason.to_string()),
                created_at: Set(Utc::now()),
                ..Default::default()
            };

            new_note.insert(txn).await
                .map_err(|e| OrderError::DatabaseError(e.to_string()))?;

            Ok(updated_order)
        })
    }).await;

    transaction_result
}

// Extend the OrderError enum to include an error type
#[derive(thiserror::Error, Debug)]
pub enum OrderError {
    #[error("Order {0} not found")]
    NotFound(i32),
    #[error("Cannot cancel order {0} in current status")]
    InvalidStatus(i32),
    #[error("Database error: {0}")]
    DatabaseError(String),
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