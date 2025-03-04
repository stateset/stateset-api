use std::sync::Arc;
use sea_orm::*;
use crate::{
    db::DbPool,
    errors::OrderError,
    events::{Event, EventSender},
    models::{
        order_entity::{self, Entity as Order},
        order_note_entity,
        OrderStatus,
    },
};
use serde::{Deserialize, Serialize};
use tracing::{error, info, instrument};
use uuid::Uuid;
use validator::Validate;
use prometheus::IntCounter;
use lazy_static::lazy_static;
use chrono::{DateTime, Utc};

lazy_static! {
    static ref ORDER_HOLDS: IntCounter = 
        IntCounter::new("order_holds", "Number of orders put on hold")
            .expect("metric can be created");
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct HoldOrderCommand {
    pub order_id: Uuid,
    #[validate(length(min = 1, max = 500, message = "Reason must be between 1 and 500 characters"))]
    pub reason: String,
    pub version: i32,  // For optimistic locking
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HoldOrderResult {
    pub id: Uuid,
    pub status: String,
    pub version: i32,
    pub hold_reason: String,
    pub updated_at: DateTime<Utc>,
}

#[async_trait::async_trait]
impl Command for HoldOrderCommand {
    type Result = HoldOrderResult;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(
        &self,
        db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, OrderError> {
        self.validate().map_err(|e| {
            let msg = format!("Invalid input: {}", e);
            error!("{}", msg);
            OrderError::ValidationError(msg)
        })?;

        let db = db_pool.as_ref();

        let updated_order = self.hold_order_in_db(db).await?;

        self.log_and_trigger_event(&event_sender, &updated_order).await?;

        ORDER_HOLDS.inc();

        Ok(HoldOrderResult {
            id: updated_order.id,
            status: updated_order.status,
            version: updated_order.version,
            hold_reason: self.reason.clone(),
            updated_at: updated_order.updated_at.and_utc(),
        })
    }
}

impl HoldOrderCommand {
    async fn hold_order_in_db(
        &self,
        db: &DatabaseConnection,
    ) -> Result<order_entity::Model, OrderError> {
        db.transaction::<_, order_entity::Model, OrderError>(|txn| {
            Box::pin(async move {
                let order = Order::find_by_id(self.order_id)
                    .one(txn)
                    .await
                    .map_err(|e| OrderError::DatabaseError(e.to_string()))?
                    .ok_or(OrderError::NotFound(self.order_id))?;

                if order.version != self.version {
                    return Err(OrderError::ConcurrentModification(self.order_id));
                }

                let mut order: order_entity::ActiveModel = order.into();
                order.status = Set(OrderStatus::OnHold.to_string());
                order.version = Set(self.version + 1);
                order.updated_at = Set(Utc::now().naive_utc());

                let updated_order = order.update(txn).await
                    .map_err(|e| OrderError::DatabaseError(e.to_string()))?;

                let new_note = order_note_entity::ActiveModel {
                    order_id: Set(self.order_id),
                    note: Set(self.reason.clone()),
                    created_at: Set(Utc::now().naive_utc()),
                    ..Default::default()
                };

                new_note.insert(txn).await
                    .map_err(|e| OrderError::DatabaseError(e.to_string()))?;

                Ok(updated_order)
            })
        }).await
    }

    async fn log_and_trigger_event(
        &self,
        event_sender: &EventSender,
        updated_order: &order_entity::Model,
    ) -> Result<(), OrderError> {
        info!(
            order_id = %self.order_id,
            reason = %self.reason,
            "Order put on hold successfully"
        );

        event_sender
            .send(Event::OrderOnHold(self.order_id))
            .await
            .map_err(|e| {
                let msg = format!("Failed to send event for order on hold: {}", e);
                error!("{}", msg);
                OrderError::EventError(msg)
            })
    }
}

// Updated OrderError enum
// Using the OrderError from crate::errors