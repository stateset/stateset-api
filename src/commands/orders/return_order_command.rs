use async_trait::async_trait;
use serde::{Serialize, Deserialize};
use std::sync::Arc;
use sea_orm::*;
use crate::{errors::ServiceError, db::DbPool, models::{
    order_entity, order_entity::Entity as Order,
    return_item_entity, return_item_entity::Entity as ReturnItem,
    order_note_entity, order_note_entity::Entity as OrderNote,
    OrderStatus
}};
use crate::events::{Event, EventSender};
use validator::Validate;
use tracing::{info, error, instrument};
use prometheus::IntCounter;
use lazy_static::lazy_static;

lazy_static! {
    static ref ORDER_RETURNS: IntCounter = 
        IntCounter::new("order_returns_total", "Total number of order returns")
            .expect("metric can be created");

    static ref ORDER_RETURN_FAILURES: IntCounter = 
        IntCounter::new("order_return_failures_total", "Total number of failed order returns")
            .expect("metric can be created");
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct ReturnOrderCommand {
    pub order_id: i32,
    #[validate(length(min = 1))]
    pub reason: String,
    #[validate(length(min = 1))]
    pub items: Vec<return_item_entity::Model>,
}

#[async_trait]
impl Command for ReturnOrderCommand {
    type Result = order_entity::Model;

    #[instrument(skip(db_pool, event_sender))]
    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        let db = db_pool.get().map_err(|e| {
            ORDER_RETURN_FAILURES.inc();
            error!("Failed to get database connection: {}", e);
            ServiceError::DatabaseError
        })?;

        let result = db.transaction::<_, order_entity::Model, ServiceError>(|txn| {
            Box::pin(async move {
                // Update order status to Returned
                let order = Order::find_by_id(self.order_id)
                    .one(txn)
                    .await
                    .map_err(|e| {
                        ORDER_RETURN_FAILURES.inc();
                        error!("Failed to find order with ID {}: {}", self.order_id, e);
                        ServiceError::DatabaseError
                    })?
                    .ok_or_else(|| {
                        ORDER_RETURN_FAILURES.inc();
                        error!("Order with ID {} not found", self.order_id);
                        ServiceError::NotFound
                    })?;

                let mut order: order_entity::ActiveModel = order.into();
                order.status = Set(OrderStatus::Returned.to_string());

                let updated_order = order.update(txn).await.map_err(|e| {
                    ORDER_RETURN_FAILURES.inc();
                    error!("Failed to update order status to Returned for order ID {}: {}", self.order_id, e);
                    ServiceError::DatabaseError
                })?;

                // Insert return items
                for item in &self.items {
                    let return_item = return_item_entity::ActiveModel {
                        order_id: Set(self.order_id),
                        product_id: Set(item.product_id),
                        quantity: Set(item.quantity),
                        ..Default::default()
                    };
                    return_item.insert(txn).await.map_err(|e| {
                        ORDER_RETURN_FAILURES.inc();
                        error!("Failed to insert return item for order ID {}: {}", self.order_id, e);
                        ServiceError::DatabaseError
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
                    error!("Failed to insert return note for order ID {}: {}", self.order_id, e);
                    ServiceError::DatabaseError
                })?;

                Ok(updated_order)
            })
        }).await?;

        // Trigger an event
        if let Err(e) = event_sender.send(Event::OrderReturned(self.order_id)).await {
            ORDER_RETURN_FAILURES.inc();
            error!("Failed to send OrderReturned event for order ID {}: {}", self.order_id, e);
            return Err(ServiceError::EventError(e.to_string()));
        }

        ORDER_RETURNS.inc();

        info!(
            order_id = %self.order_id,
            "Order returned successfully"
        );

        Ok(result)
    }
}