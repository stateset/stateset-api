use async_trait::async_trait;;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info, instrument};
use sea_orm::*;

use crate::{
    db::DbPool,
    errors::ServiceError,
    events::{Event, EventSender},
    models::{order_entity, order_entity::Entity as Order, order_note_entity, order_note_entity::Entity as OrderNote},
};

pub struct RefundOrderCommand {
    pub order_id: i32,
    pub refund_amount: f64,
    pub reason: String,
}

#[async_trait]
impl Command for RefundOrderCommand {
    type Result = order_entity::Model;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        let db = db_pool.get().map_err(|e| {
            error!("Failed to get database connection: {:?}", e);
            ServiceError::DatabaseError
        })?;

        let updated_order = db.transaction::<_, order_entity::Model, ServiceError>(|txn| {
            Box::pin(async move {
                let order = self.process_refund(txn).await?;
                self.log_refund_reason(txn).await?;
                Ok(order)
            })
        }).await.map_err(|e| {
            error!("Failed to process refund for order ID {}: {:?}", self.order_id, e);
            e
        })?;

        self.log_and_trigger_event(event_sender, &updated_order).await?;

        Ok(updated_order)
    }
}

impl RefundOrderCommand {
    async fn process_refund(&self, txn: &DatabaseTransaction) -> Result<order_entity::Model, ServiceError> {
        let order = Order::find_by_id(self.order_id)
            .one(txn)
            .await
            .map_err(|e| {
                error!("Failed to find order: {:?}", e);
                ServiceError::DatabaseError
            })?
            .ok_or_else(|| {
                error!("Order not found: {}", self.order_id);
                ServiceError::NotFound
            })?;

        let mut order: order_entity::ActiveModel = order.into();
        let new_total = order.total_amount.unwrap() - self.refund_amount;
        order.total_amount = Set(new_total);

        order.update(txn).await.map_err(|e| {
            error!("Failed to update order amount for refund: {:?}", e);
            ServiceError::DatabaseError
        })
    }

    async fn log_refund_reason(&self, txn: &DatabaseTransaction) -> Result<(), ServiceError> {
        let new_note = order_note_entity::ActiveModel {
            order_id: Set(self.order_id),
            note: Set(format!("Refunded: {} - Reason: {}", self.refund_amount, self.reason)),
            ..Default::default()
        };

        new_note.insert(txn).await.map_err(|e| {
            error!("Failed to log refund reason for order ID {}: {:?}", self.order_id, e);
            ServiceError::DatabaseError
        })?;

        Ok(())
    }

    async fn log_and_trigger_event(
        &self,
        event_sender: Arc<EventSender>,
        updated_order: &order_entity::Model,
    ) -> Result<(), ServiceError> {
        info!("Order ID {} refunded with amount: {}", self.order_id, self.refund_amount);

        event_sender
            .send(Event::OrderRefunded(self.order_id))
            .await
            .map_err(|e| {
                error!("Failed to send OrderRefunded event for order ID {}: {:?}", self.order_id, e);
                ServiceError::EventError(e.to_string())
            })
    }
}