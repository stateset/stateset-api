use uuid::Uuid;
use crate::commands::Command;
use crate::events::{Event, EventSender};
use crate::{
    db::DbPool,
    errors::ServiceError,
    models::{
        order_entity, order_entity::Entity as Order, order_note_entity, return_item_entity,
        return_item_entity::Entity as ReturnItem, OrderStatus,
    },
};
use async_trait::async_trait;
use lazy_static::lazy_static;
use prometheus::IntCounter;
use sea_orm::{*, Set};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info, instrument};
use validator::Validate;
use chrono::Utc;

lazy_static! {
    static ref ORDER_RETURNS: IntCounter =
        IntCounter::new("order_returns_total", "Total number of order returns")
            .expect("metric can be created");
    static ref ORDER_RETURN_FAILURES: IntCounter = IntCounter::new(
        "order_return_failures_total",
        "Total number of failed order returns"
    )
    .expect("metric can be created");
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct ReturnOrderCommand {
    pub order_id: Uuid,
    #[validate(length(min = 1))]
    pub reason: String,
    #[validate(length(min = 1))]
    pub items: Vec<return_item_entity::Model>,
    pub return_all: bool,
}

#[async_trait]
impl Command for ReturnOrderCommand {
    type Result = order_entity::Model;

    #[instrument(skip(db_pool, event_sender))]
    async fn execute(
        &self,
        db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        self.validate()
            .map_err(|e| ServiceError::ValidationError(e.to_string()))?;

        let db = db_pool.as_ref();

        let result = db
            .transaction::<_, order_entity::Model, ServiceError>(|txn| {
                Box::pin(async move {
                    // Update order status to Processing (returns are being processed)
                    let order = Order::find_by_id(self.order_id)
                        .one(txn)
                        .await
                        .map_err(|e| {
                            ORDER_RETURN_FAILURES.inc();
                            error!("Failed to find order with ID {}: {}", self.order_id, e);
                            ServiceError::db_error(e)
                        })?
                        .ok_or_else(|| {
                            ORDER_RETURN_FAILURES.inc();
                            error!("Order with ID {} not found", self.order_id);
                            ServiceError::NotFound
                        })?;

                    let mut order_active: order_entity::ActiveModel = order.into();
                    if self.return_all {
                        order_active.status = Set(OrderStatus::Processing.to_string());
                    }
                    order_active.updated_at = Set(Utc::now());

                    let updated_order = order_active.update(txn).await.map_err(|e| {
                        ORDER_RETURN_FAILURES.inc();
                        error!(
                            "Failed to update order status to Returned for order ID {}: {}",
                            self.order_id, e
                        );
                        ServiceError::db_error(e)
                    })?;

                    // Insert return items
                    for item in &self.items {
                        let return_item = return_item_entity::ActiveModel {
                            id: Set(Uuid::new_v4()),
                            return_id: Set(Uuid::new_v4()), // This should be the actual return ID
                            order_item_id: Set(item.order_id),
                            sku: Set("".to_string()), // Should be fetched from product
                            product_name: Set("".to_string()), // Should be fetched from product
                            quantity: Set(item.quantity),
                            unit_price: Set(0.0), // Should be fetched from order item
                            reason: Set(item.reason.clone()),
                            condition: Set("Good".to_string()),
                            restock_eligible: Set(true),
                            restocked: Set(false),
                            created_at: Set(Utc::now()),
                            updated_at: Set(Utc::now()),
                            ..Default::default()
                        };
                        return_item.insert(txn).await.map_err(|e| {
                            ORDER_RETURN_FAILURES.inc();
                            error!(
                                "Failed to insert return item for order ID {}: {}",
                                self.order_id, e
                            );
                            ServiceError::db_error(e)
                        })?;
                    }

                    // Log the return reason
                    let order_note = order_note_entity::ActiveModel {
                        order_id: Set(self.order_id),
                        note: Set(self.reason.clone()),
                        ..Default::default()
                    };
                    order_note.insert(txn).await.map_err(|e| {
                        ORDER_RETURN_FAILURES.inc();
                        error!(
                            "Failed to insert return note for order ID {}: {}",
                            self.order_id, e
                        );
                        ServiceError::db_error(e)
                    })?;

                    Ok(updated_order)
                })
            })
            .await?;

        // Send event
        event_sender
            .send(Event::OrderUpdated {
                order_id: self.order_id,
                checkout_session_id: None,
                status: None,
                refunds: vec![],
            })
            .await
            .map_err(|e| ServiceError::EventError(e.to_string()))?;

        ORDER_RETURNS.inc();

        info!(
            order_id = %self.order_id,
            "Order returned successfully"
        );

        Ok(result)
    }
}
