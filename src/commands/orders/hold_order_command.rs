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
use chrono::{DateTime, Utc};
use lazy_static::lazy_static;
use prometheus::IntCounter;
use sea_orm::{prelude::Uuid, *};
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
pub struct HoldOrderCommand {
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
pub struct HoldOrderResult {
    pub id: Uuid,
    pub order_number: String,
    pub status: String,
    pub hold_reason: String,
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

        self.log_and_trigger_event(&event_sender).await?;

        ORDER_HOLDS.inc();

        Ok(HoldOrderResult {
            id: updated_order.id,
            order_number: updated_order.order_number,
            status: updated_order.status.to_string(),
            hold_reason: self.reason.clone(),
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
                    .await?
                    .ok_or_else(|| {
                        let msg = format!("Order {} not found", self.order_id);
                        error!("{}", msg);
                        ServiceError::NotFound(msg)
                    })?;

                if order.version != self.version {
                    return Err(OrderError::ConcurrentModification(self.order_id));
                }

                let mut order: order_entity::ActiveModel = order.into();
                order.status = Set(OrderStatus::OnHold);
                order.notes = Set(Some(self.reason.clone()));
                order.updated_at = Set(Utc::now());
                order.version = Set(order.version.unwrap_or(0) + 1);

                let updated_order = order.update(txn).await?;

                let new_note = order_note_entity::ActiveModel {
                    order_id: Set(self.order_id),
                    note: Set(self.reason.clone()),
                    created_at: Set(Utc::now().naive_utc()),
                    ..Default::default()
                };

                new_note.insert(txn).await?;

                Ok(updated_order)
            })
        })
        .await
        .map_err(|e| match e {
            TransactionError::Connection(db_err) => db_err.into(),
            TransactionError::Transaction(service_err) => service_err,
        })
    }

    async fn log_and_trigger_event(
        &self,
        event_sender: &EventSender,
    ) -> Result<(), OrderError> {
        info!(
            order_id = %self.order_id,
            reason = %self.reason,
            "Order put on hold successfully"
        );

        event_sender
            .send(Event::OrderOnHold { order_id: self.order_id, reason: self.reason.clone() })
            .await
            .map_err(|e| {
                let msg = format!("Failed to send event for order on hold: {}", e);
                error!("{}", msg);
                OrderError::EventError(msg)
            })?;

        Ok(())
    }
}

// Updated OrderError enum
// Using the OrderError from crate::errors
