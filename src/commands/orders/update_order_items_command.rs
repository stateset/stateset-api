use uuid::Uuid;
use async_trait::async_trait;
use sea_orm::{*, Set};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info, instrument};
use validator::Validate;

use crate::{
    commands::Command,
    db::DbPool,
    errors::ServiceError,
    events::{Event, EventSender},
    models::{
        order_entity, order_entity::Entity as Order, order_item_entity,
        order_item_entity::Entity as OrderItem,
    },
};
use lazy_static::lazy_static;
use prometheus::IntCounter;

lazy_static! {
    static ref ORDER_ITEM_UPDATES: IntCounter = IntCounter::new(
        "order_item_updates_total",
        "Total number of order item updates"
    )
    .expect("metric can be created");
    static ref ORDER_ITEM_UPDATE_FAILURES: IntCounter = IntCounter::new(
        "order_item_update_failures_total",
        "Total number of failed order item updates"
    )
    .expect("metric can be created");
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct UpdateOrderItemsCommand {
    pub order_id: Uuid,
    #[validate(length(min = 1))]
    pub items: Vec<order_item_entity::Model>,
}

#[async_trait]
impl Command for UpdateOrderItemsCommand {
    type Result = order_entity::Model;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(
        &self,
        db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        let db = &**db_pool;

        let updated_order = db
            .transaction::<_, order_entity::Model, ServiceError>(|txn| {
                Box::pin(async move {
                    self.delete_existing_items(txn).await?;
                    self.insert_new_items(txn).await?;
                    self.recalculate_order_total(txn).await
                })
            })
            .await
            .map_err(|e| {
                error!(
                    "Transaction failed for updating order items in order ID {}: {}",
                    self.order_id, e
                );
                ORDER_ITEM_UPDATE_FAILURES.inc();
                e
            })?;

        self.log_and_trigger_event(event_sender, &updated_order)
            .await?;

        ORDER_ITEM_UPDATES.inc();
        info!(order_id = %self.order_id, "Order items updated successfully");

        Ok(updated_order)
    }
}

impl UpdateOrderItemsCommand {
    async fn delete_existing_items(&self, txn: &DatabaseTransaction) -> Result<(), ServiceError> {
        OrderItem::delete_many()
            .filter(order_item_entity::Column::OrderId.eq(self.order_id))
            .exec(txn)
            .await
            .map_err(|e| {
                error!(
                    "Failed to delete order items for order ID {}: {}",
                    self.order_id, e
                );
                ORDER_ITEM_UPDATE_FAILURES.inc();
                ServiceError::DatabaseError(e)
            })?;
        Ok(())
    }

    async fn insert_new_items(&self, txn: &DatabaseTransaction) -> Result<(), ServiceError> {
        for item in &self.items {
            let new_item = order_item_entity::ActiveModel {
                order_id: Set(self.order_id),
                product_id: Set(item.product_id),
                quantity: Set(item.quantity),
                // Set other fields as needed
                ..Default::default()
            };
            new_item.insert(txn).await.map_err(|e| {
                error!(
                    "Failed to insert order item for order ID {}: {}",
                    self.order_id, e
                );
                ORDER_ITEM_UPDATE_FAILURES.inc();
                ServiceError::DatabaseError(e)
            })?;
        }
        Ok(())
    }

    async fn recalculate_order_total(
        &self,
        txn: &DatabaseTransaction,
    ) -> Result<order_entity::Model, ServiceError> {
        // Implement the logic to recalculate the order total
        let order = Order::find_by_id(self.order_id)
            .one(txn)
            .await
            .map_err(|e| {
                error!("Failed to find order for ID {}: {}", self.order_id, e);
                ServiceError::DatabaseError(e)
            })?
            .ok_or_else(|| {
                error!("Order not found for ID {}", self.order_id);
                ServiceError::NotFound(format!("Order {} not found", self.order_id))
            })?;

        // Here you would implement the logic to recalculate the total
        // For example:
        // let total = calculate_total(&self.items);
        // let mut order: order_entity::ActiveModel = order.into();
        // order.total = Set(total);
        // order.update(txn).await.map_err(|e| {
        //     error!("Failed to update order total for ID {}: {}", self.order_id, e);
        // })

        Ok(order)
    }

    async fn log_and_trigger_event(
        &self,
        event_sender: &EventSender,
        _updated_order: &order_entity::Model,
    ) -> Result<(), ServiceError> {
        if let Err(e) = event_sender.send(Event::OrderUpdated(self.order_id)).await {
            ORDER_ITEM_UPDATE_FAILURES.inc();
            error!(
                "Failed to send OrderUpdated event for order ID {}: {}",
                self.order_id, e
            );
            return Err(ServiceError::EventError(e.to_string()));
        }
        Ok(())
    }
}
