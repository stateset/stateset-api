use async_trait::async_trait;;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info, instrument};
use sea_orm::*;

use crate::{
    db::DbPool,
    errors::ServiceError,
    events::{Event, EventSender},
    models::{order_entity, order_entity::Entity as Order, order_item_entity, order_item_entity::Entity as OrderItem},
};
use chrono::{DateTime, Utc};

pub struct SplitOrderCommand {
    pub order_id: i32,
    pub split_criteria: SplitCriteria,
}

#[async_trait]
impl Command for SplitOrderCommand {
    type Result = Vec<order_entity::Model>;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        let db = db_pool.get().map_err(|e| {
            error!("Failed to get database connection: {:?}", e);
            ServiceError::DatabaseError
        })?;

        let split_orders = db.transaction::<_, Vec<order_entity::Model>, ServiceError>(|txn| {
            Box::pin(async move {
                self.split_order_logic(txn).await
            })
        }).await.map_err(|e| {
            error!("Failed to split order ID {}: {:?}", self.order_id, e);
            e
        })?;

        self.log_and_trigger_events(event_sender, &split_orders).await?;

        Ok(split_orders)
    }
}

impl SplitOrderCommand {
    async fn split_order_logic(&self, txn: &DatabaseTransaction) -> Result<Vec<order_entity::Model>, ServiceError> {
        // Fetch the original order
        let original_order = Order::find_by_id(self.order_id)
            .one(txn)
            .await
            .map_err(|e| {
                error!("Failed to fetch original order {}: {:?}", self.order_id, e);
                ServiceError::DatabaseError
            })?
            .ok_or_else(|| {
                error!("Order {} not found", self.order_id);
                ServiceError::NotFound
            })?;

        // Fetch all items for the original order
        let order_items = OrderItem::find()
            .filter(order_item_entity::Column::OrderId.eq(self.order_id))
            .all(txn)
            .await
            .map_err(|e| {
                error!("Failed to fetch items for order {}: {:?}", self.order_id, e);
                ServiceError::DatabaseError
            })?;

        // Apply split criteria to items (this is a placeholder, implement your own logic)
        let (items_for_new_order, remaining_items) = self.apply_split_criteria(&order_items);

        // Create a new order
        let new_order = order_entity::ActiveModel {
            customer_id: Set(original_order.customer_id),
            status: Set("Pending".to_string()),
            created_at: Set(Utc::now().naive_utc()),
            // Set other fields as needed
            ..Default::default()
        };

        let new_order = new_order.insert(txn).await.map_err(|e| {
            error!("Failed to create new order: {:?}", e);
            ServiceError::DatabaseError
        })?;

        // Move items to the new order
        for item in items_for_new_order {
            let mut item: order_item_entity::ActiveModel = item.into();
            item.order_id = Set(new_order.id);
            item.update(txn).await.map_err(|e| {
                error!("Failed to update item for new order: {:?}", e);
                ServiceError::DatabaseError
            })?;
        }

        // Update the original order (e.g., recalculate totals)
        let mut original_order: order_entity::ActiveModel = original_order.into();
        // Update fields as needed based on remaining items
        original_order.update(txn).await.map_err(|e| {
            error!("Failed to update original order: {:?}", e);
            ServiceError::DatabaseError
        })?;

        Ok(vec![original_order.try_into_model()?, new_order])
    }

    fn apply_split_criteria(&self, items: &[order_item_entity::Model]) -> (Vec<order_item_entity::Model>, Vec<order_item_entity::Model>) {
        // Placeholder for actual split criteria logic
        // Implement your own logic to split items based on self.split_criteria
        let split_point = items.len() / 2;
        let items_for_new_order = items[..split_point].to_vec();
        let remaining_items = items[split_point..].to_vec();
        (items_for_new_order, remaining_items)
    }

    async fn log_and_trigger_events(
        &self,
        event_sender: Arc<EventSender>,
        split_orders: &[order_entity::Model],
    ) -> Result<(), ServiceError> {
        for order in split_orders {
            info!("Order ID {} split into new order ID {}", self.order_id, order.id);
            event_sender
                .send(Event::OrderSplit(order.id))
                .await
                .map_err(|e| {
                    error!("Failed to send OrderSplit event for order ID {}: {:?}", order.id, e);
                    ServiceError::EventError(e.to_string())
                })?;
        }
        Ok(())
    }
}