use async_trait::async_trait;;
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

pub struct MergeOrdersCommand {
    pub order_ids: Vec<i32>,
}

#[async_trait]
impl Command for MergeOrdersCommand {
    type Result = order_entity::Model;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        let db = db_pool.get().map_err(|e| {
            error!("Failed to get database connection: {:?}", e);
            ServiceError::DatabaseError
        })?;

        let merged_order = db.transaction::<_, order_entity::Model, ServiceError>(|txn| {
            Box::pin(async move {
                self.merge_orders(txn).await
            })
        }).await.map_err(|e| {
            error!("Failed to merge orders {:?}: {:?}", self.order_ids, e);
            e
        })?;

        self.log_and_trigger_event(event_sender, &merged_order).await?;

        Ok(merged_order)
    }
}

impl MergeOrdersCommand {
    async fn merge_orders(&self, txn: &DatabaseTransaction) -> Result<order_entity::Model, ServiceError> {
        // Fetch all orders to be merged
        let orders = Order::find()
            .filter(order_entity::Column::Id.is_in(self.order_ids.clone()))
            .all(txn)
            .await
            .map_err(|e| {
                error!("Failed to fetch orders: {:?}", e);
                ServiceError::DatabaseError
            })?;

        if orders.len() != self.order_ids.len() {
            return Err(ServiceError::NotFound);
        }

        // Create a new merged order
        let new_order = order_entity::ActiveModel {
            status: Set("Pending".to_string()),
            created_at: Set(Utc::now().naive_utc()),
            // Set other fields as needed
            ..Default::default()
        };

        let merged_order = new_order.insert(txn).await.map_err(|e| {
            error!("Failed to create merged order: {:?}", e);
            ServiceError::DatabaseError
        })?;

        // Merge order items
        for order in orders {
            let items = OrderItem::find()
                .filter(order_item_entity::Column::OrderId.eq(order.id))
                .all(txn)
                .await
                .map_err(|e| {
                    error!("Failed to fetch order items: {:?}", e);
                    ServiceError::DatabaseError
                })?;

            for item in items {
                let new_item = order_item_entity::ActiveModel {
                    order_id: Set(merged_order.id),
                    product_id: Set(item.product_id),
                    quantity: Set(item.quantity),
                    // Set other fields as needed
                    ..Default::default()
                };

                new_item.insert(txn).await.map_err(|e| {
                    error!("Failed to insert merged order item: {:?}", e);
                    ServiceError::DatabaseError
                })?;
            }

            // Delete the old order
            Order::delete_by_id(order.id).exec(txn).await.map_err(|e| {
                error!("Failed to delete old order: {:?}", e);
                ServiceError::DatabaseError
            })?;
        }

        Ok(merged_order)
    }

    async fn log_and_trigger_event(
        &self,
        event_sender: Arc<EventSender>,
        merged_order: &order_entity::Model,
    ) -> Result<(), ServiceError> {
        info!("Orders merged: {:?}", self.order_ids);

        event_sender
            .send(Event::OrdersMerged(self.order_ids.clone()))
            .await
            .map_err(|e| {
                error!("Failed to send OrdersMerged event: {:?}", e);
                ServiceError::EventError(e.to_string())
            })
    }
}