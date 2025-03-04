use async_trait::async_trait;
use serde::{Serialize, Deserialize};
use std::sync::Arc;
use sea_orm::*;
use crate::{errors::OrderError, db::DbPool, models::{order_entity, order_entity::Entity as Order, order_note_entity, OrderStatus}};
use crate::events::{Event, EventSender};
use validator::Validate;
use tracing::{info, error, instrument};
use chrono::Utc;
use prometheus::IntCounter;
use lazy_static::lazy_static;

lazy_static! {
    static ref ORDER_HOLDS: IntCounter = 
        IntCounter::new("order_holds", "Number of orders put on hold")
            .expect("metric can be created");
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct ArchiveOrderCommand {
    pub order_id: uuid::Uuid,
    #[validate(length(min = 1, max = 500, message = "Reason must be between 1 and 500 characters"))]
    pub reason: String,
    pub version: i32,  // Added for optimistic locking
}

#[async_trait]
impl Command for ArchiveOrderCommand {
    type Result = order_entity::Model;

    #[instrument(skip(db_pool, event_sender))]
    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, OrderError> {
        let db = db_pool.get().map_err(|e| OrderError::DatabaseError(e.to_string()))?;

        let updated_order = archive_order_in_db(&db, self.order_id, &self.reason, self.version).await?;

        event_sender.send(Event::OrderArchived(self.order_id))
            .await
            .map_err(|e| OrderError::EventError(e.to_string()))?;

        ORDER_HOLDS.inc();

        info!(
            order_id = %self.order_id,
            reason = %self.reason,
            "Order archived successfully"
        );

        Ok(updated_order)
    }
}

async fn archive_order_in_db(db: &DatabaseConnection, order_id: uuid::Uuid, reason: &str, version: i32) -> Result<order_entity::Model, OrderError> {
    let transaction_result = db.transaction::<_, order_entity::Model, OrderError>(|txn| {
        Box::pin(async move {
            let order = Order::find_by_id(order_id)
                .one(txn)
                .await
                .map_err(|e| OrderError::DatabaseError(e.to_string()))?
                .ok_or(OrderError::NotFound(order_id))?;

            if order.version != version {
                return Err(OrderError::ConcurrentModification(order_id));
            }

            let mut order: order_entity::ActiveModel = order.into();
            order.status = Set(OrderStatus::Archived.to_string());
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

// Assuming you have defined this error type
// Using the OrderError from crate::errors
