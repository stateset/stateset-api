use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info, instrument};
use validator::Validate;
use sea_orm::*;

use crate::{
    db::DbPool,
    errors::ServiceError,
    events::{Event, EventSender},
    models::{order_entity, order_entity::Entity as Order, order_item_entity, order_item_entity::Entity as OrderItem},
};
use chrono::{DateTime, Utc};

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct PartialCancelOrderCommand {
    pub order_id: i32,

    #[validate(length(min = 1))]
    pub item_ids: Vec<i32>, // IDs of items to cancel
}

#[async_trait]
impl Command for PartialCancelOrderCommand {
    type Result = order_entity::Model;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        let db = db_pool.get().map_err(|e| {
            error!("Failed to get database connection: {:?}", e);
            ServiceError::DatabaseError
        })?;

        let updated_order = db.transaction::<_, order_entity::Model, ServiceError>(|txn| {
            Box::pin(async move {
                self.remove_items(txn).await?;
                self.recalculate_order_total(txn).await
            })
        }).await.map_err(|e| {
            error!("Failed to partially cancel order ID {}: {:?}", self.order_id, e);
            e
        })?;

        self.log_and_trigger_event(event_sender, &updated_order).await?;

        Ok(updated_order)
    }
}

impl PartialCancelOrderCommand {
    async fn remove_items(&self, txn: &DatabaseTransaction) -> Result<(), ServiceError> {
        OrderItem::delete_many()
            .filter(order_item_entity::Column::Id.is_in(self.item_ids.clone()))
            .exec(txn)
            .await
            .map_err(|e| {
                error!("Failed to remove items from order ID {}: {:?}", self.order_id, e);
                ServiceError::DatabaseError
            })?;
        Ok(())
    }

    async fn recalculate_order_total(&self, txn: &DatabaseTransaction) -> Result<order_entity::Model, ServiceError> {
        // Fetch the remaining items for the order
        let remaining_items = OrderItem::find()
            .filter(order_item_entity::Column::OrderId.eq(self.order_id))
            .all(txn)
            .await
            .map_err(|e| {
                error!("Failed to fetch remaining items for order ID {}: {:?}", self.order_id, e);
                ServiceError::DatabaseError
            })?;

        // Calculate the new total
        let new_total: f64 = remaining_items.iter().map(|item| item.price * item.quantity as f64).sum();

        // Update the order with the new total
        let order = Order::find_by_id(self.order_id)
            .one(txn)
            .await
            .map_err(|e| {
                error!("Failed to find order {}: {:?}", self.order_id, e);
                ServiceError::DatabaseError
            })?
            .ok_or_else(|| {
                error!("Order {} not found", self.order_id);
                ServiceError::NotFound
            })?;

        let mut order: order_entity::ActiveModel = order.into();
        order.total_amount = Set(new_total);

        order.update(txn).await.map_err(|e| {
            error!("Failed to update total for order ID {}: {:?}", self.order_id, e);
            ServiceError::DatabaseError
        })
    }

    async fn log_and_trigger_event(
        &self,
        event_sender: Arc<EventSender>,
        updated_order: &order_entity::Model,
    ) -> Result<(), ServiceError> {
        info!("Partial cancellation of items for order ID: {}", self.order_id);

        event_sender
            .send(Event::OrderUpdated(self.order_id))
            .await
            .map_err(|e| {
                error!("Failed to send OrderUpdated event for order ID {}: {:?}", self.order_id, e);
                ServiceError::EventError(e.to_string())
            })
    }
}