use crate::{
    commands::Command,
    db::DbPool,
    errors::{OrderError, ServiceError},
    events::{Event, EventSender},
    models::{
        order_entity::{self, Entity as Order},
        order_note_entity, OrderStatus,
    },
};
use chrono::Utc;
use lazy_static::lazy_static;
use prometheus::{IntCounter, IntCounterVec};
use sea_orm::{*, Set, TransactionError};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info, instrument, warn};
use uuid::Uuid;
use validator::Validate;

lazy_static! {
    static ref ORDER_CANCELLATIONS: IntCounter = IntCounter::new(
        "order_cancellations_total",
        "Total number of order cancellations"
    )
    .expect("metric can be created");
    static ref ORDER_CANCELLATION_FAILURES: IntCounterVec = IntCounterVec::new(
        prometheus::Opts::new(
        "order_cancellation_failures_total",
        "Total number of failed order cancellations"),
        &["error_type"]
    )
    .expect("metric can be created");
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct CancelOrderCommand {
    pub order_id: Uuid,
    #[validate(length(
        min = 1,
        max = 500,
        message = "Reason must be between 1 and 500 characters"
    ))]
    pub reason: String,
    pub version: i32, // For optimistic locking
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CancelOrderResult {
    pub id: Uuid,
    pub status: String,
    pub version: i32,
    pub cancellation_reason: String,
}

#[async_trait::async_trait]
impl Command for CancelOrderCommand {
    type Result = CancelOrderResult;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(
        &self,
        db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, OrderError> {
        self.validate().map_err(|e| {
            ORDER_CANCELLATION_FAILURES
                .with_label_values(&["validation_error"])
                .inc();
            let msg = format!("Invalid input: {}", e);
            error!("{}", msg);
            OrderError::ValidationError(msg)
        })?;

        let db = db_pool.as_ref();

        let updated_order = self.cancel_order_in_db(db).await?;

        self.log_and_trigger_event(&event_sender, &updated_order)
            .await?;

        ORDER_CANCELLATIONS.inc();

        Ok(CancelOrderResult {
            id: updated_order.id,
            status: updated_order.status.to_string(),
            version: updated_order.version,
            cancellation_reason: self.reason.clone(),
        })
    }
}

impl CancelOrderCommand {
    #[instrument(skip(db))]
    async fn cancel_order_in_db(
        &self,
        db: &DatabaseConnection,
    ) -> Result<order_entity::Model, OrderError> {
        db.transaction::<_, order_entity::Model, OrderError>(|txn| {
            Box::pin(async move {
                let order = Order::find_by_id(self.order_id)
                    .one(txn)
                    .await
                    .map_err(|e| OrderError::DatabaseError(e))?
                    .ok_or(ServiceError::NotFound(format!("Order {} not found", self.order_id)))?;

                if order.version != self.version {
                    warn!(
                        "Concurrent modification detected for order {}",
                        self.order_id
                    );
                    return Err(OrderError::ConcurrentModification(self.order_id));
                }

                let mut order: order_entity::ActiveModel = order.into();
                order.status = Set(OrderStatus::Cancelled);
                order.version = Set(self.version + 1);

                let updated_order = order
                    .update(txn)
                    .await
                    .map_err(|e| OrderError::DatabaseError(e))?;

                let new_note = order_note_entity::ActiveModel {
                    order_id: Set(self.order_id),
                    note: Set(self.reason.clone()),
                    created_at: Set(Utc::now().naive_utc()),
                    ..Default::default()
                };

                new_note
                    .insert(txn)
                    .await
                    .map_err(|e| OrderError::DatabaseError(e))?;

                Ok(updated_order)
            })
        })
        .await
        .map_err(|e| match e {
            TransactionError::Connection(db_err) => ServiceError::DatabaseError(db_err),
            TransactionError::Transaction(service_err) => service_err,
        })
    }

    async fn log_and_trigger_event(
        &self,
        event_sender: &EventSender,
        updated_order: &order_entity::Model,
    ) -> Result<(), OrderError> {
        info!(
            order_id = %self.order_id,
            reason = %self.reason,
            "Order canceled successfully"
        );

        event_sender
            .send(Event::OrderCancelled(self.order_id))
            .await
            .map_err(|e| {
                ORDER_CANCELLATION_FAILURES
                    .with_label_values(&["event_error"])
                    .inc();
                let msg = format!("Failed to send event for canceled order: {}", e);
                error!("{}", msg);
                OrderError::EventError(msg)
            })
    }
}

// Using the OrderError from crate::errors
