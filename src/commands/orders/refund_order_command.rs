use crate::{
    commands::Command,
    db::DbPool,
    errors::ServiceError,
    events::{Event, EventSender},
    models::{order_entity::{self, Entity as Order}, order_note_entity},
};
use chrono::{DateTime, Utc};
use sea_orm::{*, Set, TransactionError, TransactionTrait};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info, instrument};
use uuid::Uuid;
use validator::Validate;

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

        self.log_and_trigger_event(&event_sender, &updated_order)
            .await?;

        Ok(RefundOrderResult {
            order_id: updated_order.id,
            refunded_amount: self.refund_amount,
            new_total_amount: updated_order.total_amount,
            refund_reason: self.reason.clone(),
            refunded_at: updated_order.updated_at,
        })
    }
}

impl RefundOrderCommand {
    async fn process_refund(
        &self,
        db: &DatabaseConnection,
    ) -> Result<order_entity::Model, ServiceError> {
        let order_id = self.order_id;
        let refund_amount = self.refund_amount;
        let reason = self.reason.clone();
        
        db.transaction::<_, order_entity::Model, ServiceError>(|txn| {
            Box::pin(async move {
                // Update order amount
                let order = Order::find_by_id(order_id)
                    .one(txn)
                    .await
                    .map_err(|e| {
                        error!("Failed to find order: {}", e);
                        ServiceError::db_error(e)
                    })?
                    .ok_or_else(|| {
                        let msg = format!("Order {} not found", order_id);
                        error!("{}", msg);
                        ServiceError::NotFound(msg)
                    })?;

                if order.total_amount < refund_amount {
                    let msg = format!(
                        "Refund amount {} exceeds order total {}",
                        refund_amount, order.total_amount
                    );
                    error!("{}", msg);
                    return Err(ServiceError::InvalidOperation(msg));
                }

                let current_total = order.total_amount;
                let mut order: order_entity::ActiveModel = order.into();
                let new_total = current_total - refund_amount;
                order.total_amount = Set(new_total);
                order.updated_at = Set(Utc::now());

                let updated_order = order.update(txn).await.map_err(|e| {
                    error!("Failed to update order: {}", e);
                    ServiceError::db_error(e)
                })?;

                // Log refund reason
                let new_note = order_note_entity::ActiveModel {
                    order_id: Set(order_id),
                    note: Set(format!(
                        "Refunded: {} - Reason: {}",
                        refund_amount, reason
                    )),
                    created_at: Set(Utc::now().naive_utc()),
                    ..Default::default()
                };

                new_note.insert(txn).await.map_err(|e| {
                    error!("Failed to insert refund reason: {}", e);
                    ServiceError::db_error(e)
                })?;

                Ok(updated_order)
            })
        })
        .await
        .map_err(|e| {
            error!("Transaction failed for order refund: {}", e);
            match e {
                TransactionError::Connection(db_err) => ServiceError::db_error(db_err),
                TransactionError::Transaction(service_err) => service_err,
            }
        })
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
            .send(Event::OrderUpdated {
                order_id: self.order_id,
                checkout_session_id: None, // Orders may not have checkout_session_id
                status: Some(updated_order.status.clone()),
                refunds: vec![format!("${}", self.refund_amount)],
            })
            .await
            .map_err(|e| {
                let msg = format!("Failed to send order refunded event: {}", e);
                error!("{}", msg);
                ServiceError::EventError(msg)
            })
    }
}
