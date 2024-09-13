use std::sync::Arc;
use sea_orm::*;
use crate::{
    db::DbPool,
    errors::ServiceError,
    events::{Event, EventSender},
    models::{
        order_entity::{self, Entity as Order},
        order_item_entity::{self, Entity as OrderItem},
    },
};
use serde::{Deserialize, Serialize};
use tracing::{error, info, instrument};
use uuid::Uuid;
use validator::Validate;
use chrono::{DateTime, Utc};

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct MergeOrdersCommand {
    #[validate(length(min = 2, message = "At least two orders are required for merging"))]
    pub order_ids: Vec<Uuid>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MergeOrdersResult {
    pub merged_order_id: Uuid,
    pub merged_order_status: String,
    pub merged_item_count: usize,
    pub merged_at: DateTime<Utc>,
}

#[async_trait::async_trait]
impl Command for MergeOrdersCommand {
    type Result = MergeOrdersResult;

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

        let merged_order = self.merge_orders(db).await?;

        self.log_and_trigger_event(&event_sender, &merged_order).await?;

        Ok(MergeOrdersResult {
            merged_order_id: merged_order.id,
            merged_order_status: merged_order.status,
            merged_item_count: 0, // You might want to calculate this based on the actual items
            merged_at: merged_order.created_at.and_utc(),
        })
    }
}

impl MergeOrdersCommand {
    async fn merge_orders(&self, db: &DatabaseConnection) -> Result<order_entity::Model, ServiceError> {
        db.transaction::<_, order_entity::Model, ServiceError>(|txn| {
            Box::pin(async move {
                let orders = self.fetch_orders(txn).await?;
                let merged_order = self.create_merged_order(txn).await?;
                self.merge_order_items(txn, &orders, &merged_order).await?;
                self.delete_old_orders(txn, &orders).await?;
                Ok(merged_order)
            })
        }).await
    }

    async fn fetch_orders(&self, txn: &DatabaseTransaction) -> Result<Vec<order_entity::Model>, ServiceError> {
        let orders = Order::find()
            .filter(order_entity::Column::Id.is_in(self.order_ids.clone()))
            .all(txn)
            .await
            .map_err(|e| {
                let msg = format!("Failed to fetch orders: {}", e);
                error!("{}", msg);
                ServiceError::DatabaseError(msg)
            })?;

        if orders.len() != self.order_ids.len() {
            let msg = format!("Not all orders found: expected {}, found {}", self.order_ids.len(), orders.len());
            error!("{}", msg);
            return Err(ServiceError::NotFound(msg));
        }

        Ok(orders)
    }

    async fn create_merged_order(&self, txn: &DatabaseTransaction) -> Result<order_entity::Model, ServiceError> {
        let new_order = order_entity::ActiveModel {
            status: Set("Pending".to_string()),
            created_at: Set(Utc::now().naive_utc()),
            ..Default::default()
        };

        new_order.insert(txn).await.map_err(|e| {
            let msg = format!("Failed to create merged order: {}", e);
            error!("{}", msg);
            ServiceError::DatabaseError(msg)
        })
    }

    async fn merge_order_items(
        &self,
        txn: &DatabaseTransaction,
        orders: &[order_entity::Model],
        merged_order: &order_entity::Model,
    ) -> Result<(), ServiceError> {
        for order in orders {
            let items = OrderItem::find()
                .filter(order_item_entity::Column::OrderId.eq(order.id))
                .all(txn)
                .await
                .map_err(|e| {
                    let msg = format!("Failed to fetch order items: {}", e);
                    error!("{}", msg);
                    ServiceError::DatabaseError(msg)
                })?;

            for item in items {
                let new_item = order_item_entity::ActiveModel {
                    order_id: Set(merged_order.id),
                    product_id: Set(item.product_id),
                    quantity: Set(item.quantity),
                    ..Default::default()
                };

                new_item.insert(txn).await.map_err(|e| {
                    let msg = format!("Failed to insert merged order item: {}", e);
                    error!("{}", msg);
                    ServiceError::DatabaseError(msg)
                })?;
            }
        }

        Ok(())
    }

    async fn delete_old_orders(&self, txn: &DatabaseTransaction, orders: &[order_entity::Model]) -> Result<(), ServiceError> {
        for order in orders {
            Order::delete_by_id(order.id).exec(txn).await.map_err(|e| {
                let msg = format!("Failed to delete old order: {}", e);
                error!("{}", msg);
                ServiceError::DatabaseError(msg)
            })?;
        }

        Ok(())
    }

    async fn log_and_trigger_event(
        &self,
        event_sender: &EventSender,
        merged_order: &order_entity::Model,
    ) -> Result<(), ServiceError> {
        info!(
            merged_order_id = %merged_order.id,
            original_order_ids = ?self.order_ids,
            "Orders merged successfully"
        );

        event_sender
            .send(Event::OrdersMerged(self.order_ids.clone(), merged_order.id))
            .await
            .map_err(|e| {
                let msg = format!("Failed to send event for merged orders: {}", e);
                error!("{}", msg);
                ServiceError::EventError(msg)
            })
    }
}