use crate::{
    commands::Command,
    db::DbPool,
    errors::{ServiceError, ServiceError as OrderError},
    events::{Event, EventSender},
    models::{
        order::OrderStatus,
        order_entity::{self, Entity as Order},
        order_note_entity,
    },
};
use async_trait::async_trait;
use chrono::Utc;
use lazy_static::lazy_static;
use prometheus::IntCounter;
use sea_orm::{Set, TransactionError, TransactionTrait, *};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info, instrument};
use validator::Validate;

lazy_static! {
    static ref ORDER_HOLDS: IntCounter =
        IntCounter::new("order_holds", "Number of orders put on hold")
            .expect("metric can be created");
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct ArchiveOrderCommand {
    pub order_id: uuid::Uuid,
    #[validate(length(
        min = 1,
        max = 500,
        message = "Reason must be between 1 and 500 characters"
    ))]
    pub reason: String,
    pub version: i32, // Added for optimistic locking
}

#[async_trait]
impl Command for ArchiveOrderCommand {
    type Result = order_entity::Model;

    #[instrument(skip(db_pool, event_sender))]
    async fn execute(
        &self,
        db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        let db = db_pool.as_ref();

        let updated_order = archive_order_in_db(&db, self.order_id, &self.reason, self.version)
            .await
            .map_err(|e| ServiceError::OrderError(format!("Archive failed: {}", e)))?;

        event_sender
            .send(Event::OrderUpdated(self.order_id))
            .await
            .map_err(|e| ServiceError::EventError(e.to_string()))?;

        ORDER_HOLDS.inc();

        info!(
            order_id = %self.order_id,
            reason = %self.reason,
            "Order archived successfully"
        );

        Ok(updated_order)
    }
}

async fn archive_order_in_db(
    db: &DatabaseConnection,
    order_id: uuid::Uuid,
    reason: &str,
    version: i32,
) -> Result<order_entity::Model, OrderError> {
    let transaction_result = db
        .transaction::<_, order_entity::Model, OrderError>(|txn| {
            Box::pin(async move {
                let order = Order::find_by_id(order_id)
                    .one(txn)
                    .await
                    .map_err(|e| OrderError::DatabaseError(e))?
                    .ok_or(OrderError::NotFound(order_id.to_string()))?;

                // Check version
                if order.version != version {
                    return Err(OrderError::InvalidOperation(format!(
                        "Version mismatch for order {}",
                        order_id
                    )));
                }

                let mut order: order_entity::ActiveModel = order.into();
                order.status = Set(OrderStatus::Cancelled);
                order.updated_at = Set(Utc::now());
                order.version = Set(version + 1);

                let updated_order = order.update(txn).await.map_err(|e| {
                    error!("Failed to archive order {}: {}", order_id, e);
                    OrderError::DatabaseError(e)
                })?;

                // Create note for archival
                let note = order_note_entity::ActiveModel {
                    order_id: Set(order_id),
                    note: Set(format!("Order archived at {}", Utc::now())),
                    created_at: Set(Utc::now().naive_utc()),
                    ..Default::default()
                };

                note.insert(txn)
                    .await
                    .map_err(|e| OrderError::DatabaseError(e))?;

                Ok(updated_order)
            })
        })
        .await
        .map_err(|e| match e {
            TransactionError::Connection(db_err) => OrderError::DatabaseError(db_err),
            TransactionError::Transaction(service_err) => service_err,
        })?;

    Ok(transaction_result)
}

// Assuming you have defined this error type
// Using the OrderError from crate::errors
