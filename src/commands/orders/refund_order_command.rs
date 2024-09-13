use std::sync::Arc;
use sea_orm::*;
use crate::{
    db::DbPool,
    errors::ServiceError,
    events::{Event, EventSender},
    models::{
        order_entity::{self, Entity as Order},
        order_note_entity::{self, Entity as OrderNote},
    },
};
use serde::{Deserialize, Serialize};
use tracing::{error, info, instrument};
use uuid::Uuid;
use validator::Validate;
use chrono::{DateTime, Utc};

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct RefundOrderCommand {
    pub order_id: Uuid,
    #[validate(range(min = 0.01))]
    pub refund_amount: f64,
    #[validate(length(min = 1, max = 500))]
    pub reason: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RefundOrderResult {
    pub order_id: Uuid,
    pub refunded_amount: f64,
    pub new_total_amount: f64,
    pub refund_reason: String,
    pub refunded_at: DateTime<Utc>,
}

#[async_trait::async_trait]
impl Command for RefundOrderCommand {
    type Result = RefundOrderResult;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(
        &self,
        db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        self.validate().map_err(|e| {
            let msg = format!("Invalid input: {}", e);
            error!("{}", msg);
            ServiceError::ValidationError(msg)
        })?;

        let db = db_pool.as_ref();

        let updated_order = self.process_refund(db).await?;

        self.log_and_trigger_event(&event_sender, &updated_order).await?;

        Ok(RefundOrderResult {
            order_id: updated_order.id,
            refunded_amount: self.refund_amount,
            new_total_amount: updated_order.total_amount,
            refund_reason: self.reason.clone(),
            refunded_at: updated_order.updated_at.and_utc(),
        })
    }
}

impl RefundOrderCommand {
    async fn process_refund(&self, db: &DatabaseConnection) -> Result<order_entity::Model, ServiceError> {
        db.transaction::<_, order_entity::Model, ServiceError>(|txn| {
            Box::pin(async move {
                let updated_order = self.update_order_amount(txn).await?;
                self.log_refund_reason(txn).await?;
                Ok(updated_order)
            })
        }).await
    }

    async fn update_order_amount(&self, txn: &DatabaseTransaction) -> Result<order_entity::Model, ServiceError> {
        let order = Order::find_by_id(self.order_id)
            .one(txn)
            .await
            .map_err(|e| {
                let msg = format!("Failed to find order: {}", e);
                error!("{}", msg);
                ServiceError::DatabaseError(msg)
            })?
            .ok_or_else(|| {
                let msg = format!("Order not found: {}", self.order_id);
                error!("{}", msg);
                ServiceError::NotFound(msg)
            })?;

        if order.total_amount < self.refund_amount {
            let msg = format!("Refund amount {} exceeds order total {}", self.refund_amount, order.total_amount);
            error!("{}", msg);
            return Err(ServiceError::InvalidOperation(msg));
        }

        let mut order: order_entity::ActiveModel = order.into();
        let new_total = order.total_amount.unwrap() - self.refund_amount;
        order.total_amount = Set(new_total);
        order.updated_at = Set(Utc::now().naive_utc());

        order.update(txn).await.map_err(|e| {
            let msg = format!("Failed to update order amount for refund: {}", e);
            error!("{}", msg);
            ServiceError::DatabaseError(msg)
        })
    }

    async fn log_refund_reason(&self, txn: &DatabaseTransaction) -> Result<(), ServiceError> {
        let new_note = order_note_entity::ActiveModel {
            order_id: Set(self.order_id),
            note: Set(format!("Refunded: {} - Reason: {}", self.refund_amount, self.reason)),
            created_at: Set(Utc::now().naive_utc()),
            ..Default::default()
        };

        new_note.insert(txn).await.map_err(|e| {
            let msg = format!("Failed to log refund reason for order ID {}: {}", self.order_id, e);
            error!("{}", msg);
            ServiceError::DatabaseError(msg)
        })?;

        Ok(())
    }

    async fn log_and_trigger_event(
        &self,
        event_sender: &EventSender,
        updated_order: &order_entity::Model,
    ) -> Result<(), ServiceError> {
        info!(
            order_id = %self.order_id,
            refund_amount = %self.refund_amount,
            reason = %self.reason,
            "Order refunded successfully"
        );

        event_sender
            .send(Event::OrderRefunded(self.order_id, self.refund_amount))
            .await
            .map_err(|e| {
                let msg = format!("Failed to send event for refunded order: {}", e);
                error!("{}", msg);
                ServiceError::EventError(msg)
            })
    }
}