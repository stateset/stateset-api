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
pub struct SplitOrderCommand {
    pub order_id: Uuid,
    pub split_criteria: SplitCriteria,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum SplitCriteria {
    EvenSplit,
    // Add other split criteria as needed
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SplitOrderResult {
    pub original_order: OrderSummary,
    pub new_order: OrderSummary,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OrderSummary {
    pub id: Uuid,
    pub customer_id: Uuid,
    pub status: String,
    pub item_count: usize,
    pub created_at: DateTime<Utc>,
}

#[async_trait::async_trait]
impl Command for SplitOrderCommand {
    type Result = SplitOrderResult;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(
        &self,
        db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        let db = db_pool.as_ref();

        let split_orders = self.split_order(db).await?;

        self.log_and_trigger_events(&event_sender, &split_orders).await?;

        Ok(SplitOrderResult {
            original_order: OrderSummary::from(&split_orders[0]),
            new_order: OrderSummary::from(&split_orders[1]),
        })
    }
}

impl SplitOrderCommand {
    async fn split_order(
        &self,
        db: &DatabaseConnection,
    ) -> Result<Vec<order_entity::Model>, ServiceError> {
        db.transaction::<_, Vec<order_entity::Model>, ServiceError>(|txn| {
            Box::pin(async move {
                let original_order = self.fetch_original_order(txn).await?;
                let order_items = self.fetch_order_items(txn).await?;

                let (items_for_new_order, remaining_items) = self.apply_split_criteria(&order_items);

                let new_order = self.create_new_order(txn, &original_order).await?;
                self.move_items_to_new_order(txn, &new_order, items_for_new_order).await?;
                let updated_original_order = self.update_original_order(txn, original_order, remaining_items).await?;

                Ok(vec![updated_original_order, new_order])
            })
        }).await
    }

    async fn fetch_original_order(&self, txn: &DatabaseTransaction) -> Result<order_entity::Model, ServiceError> {
        Order::find_by_id(self.order_id)
            .one(txn)
            .await
            .map_err(|e| {
                let msg = format!("Failed to fetch original order {}: {}", self.order_id, e);
                error!("{}", msg);
                ServiceError::DatabaseError(msg)
            })?
            .ok_or_else(|| {
                let msg = format!("Order {} not found", self.order_id);
                error!("{}", msg);
                ServiceError::NotFound(msg)
            })
    }

    async fn fetch_order_items(&self, txn: &DatabaseTransaction) -> Result<Vec<order_item_entity::Model>, ServiceError> {
        OrderItem::find()
            .filter(order_item_entity::Column::OrderId.eq(self.order_id))
            .all(txn)
            .await
            .map_err(|e| {
                let msg = format!("Failed to fetch items for order {}: {}", self.order_id, e);
                error!("{}", msg);
                ServiceError::DatabaseError(msg)
            })
    }

    fn apply_split_criteria(&self, items: &[order_item_entity::Model]) -> (Vec<order_item_entity::Model>, Vec<order_item_entity::Model>) {
        match self.split_criteria {
            SplitCriteria::EvenSplit => {
                let split_point = items.len() / 2;
                let items_for_new_order = items[..split_point].to_vec();
                let remaining_items = items[split_point..].to_vec();
                (items_for_new_order, remaining_items)
            },
            // Implement other split criteria as needed
        }
    }

    async fn create_new_order(&self, txn: &DatabaseTransaction, original_order: &order_entity::Model) -> Result<order_entity::Model, ServiceError> {
        let new_order = order_entity::ActiveModel {
            customer_id: Set(original_order.customer_id),
            status: Set("Pending".to_string()),
            created_at: Set(Utc::now().naive_utc()),
            ..Default::default()
        };

        new_order.insert(txn).await.map_err(|e| {
            let msg = format!("Failed to create new order: {}", e);
            error!("{}", msg);
            ServiceError::DatabaseError(msg)
        })
    }

    async fn move_items_to_new_order(
        &self,
        txn: &DatabaseTransaction,
        new_order: &order_entity::Model,
        items: Vec<order_item_entity::Model>,
    ) -> Result<(), ServiceError> {
        for item in items {
            let mut item: order_item_entity::ActiveModel = item.into();
            item.order_id = Set(new_order.id);
            item.update(txn).await.map_err(|e| {
                let msg = format!("Failed to update item for new order: {}", e);
                error!("{}", msg);
                ServiceError::DatabaseError(msg)
            })?;
        }
        Ok(())
    }

    async fn update_original_order(
        &self,
        txn: &DatabaseTransaction,
        original_order: order_entity::Model,
        remaining_items: Vec<order_item_entity::Model>,
    ) -> Result<order_entity::Model, ServiceError> {
        let mut original_order: order_entity::ActiveModel = original_order.into();
        // Update fields as needed based on remaining items
        // For example, you might want to update the total price or item count
        original_order.update(txn).await.map_err(|e| {
            let msg = format!("Failed to update original order: {}", e);
            error!("{}", msg);
            ServiceError::DatabaseError(msg)
        })
    }

    async fn log_and_trigger_events(
        &self,
        event_sender: &EventSender,
        split_orders: &[order_entity::Model],
    ) -> Result<(), ServiceError> {
        for order in split_orders {
            info!("Order ID {} split into new order ID {}", self.order_id, order.id);
            event_sender
                .send(Event::OrderSplit(order.id))
                .await
                .map_err(|e| {
                    let msg = format!("Failed to send OrderSplit event for order ID {}: {}", order.id, e);
                    error!("{}", msg);
                    ServiceError::EventError(msg)
                })?;
        }
        Ok(())
    }
}

impl From<&order_entity::Model> for OrderSummary {
    fn from(order: &order_entity::Model) -> Self {
        OrderSummary {
            id: order.id,
            customer_id: order.customer_id,
            status: order.status.clone(),
            item_count: 0, // You might want to calculate this based on the actual items
            created_at: order.created_at.and_utc(),
        }
    }
}