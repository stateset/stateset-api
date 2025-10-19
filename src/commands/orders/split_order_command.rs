use crate::{
    commands::Command,
    db::DbPool,
    errors::ServiceError,
    events::{Event, EventSender},
    models::{
        order::OrderStatus,
        order_entity::{self, Entity as Order},
        order_item_entity::{self, Entity as OrderItem},
        order_note_entity::{self, Entity as OrderNote},
    },
};
use chrono::{DateTime, Utc};
use sea_orm::{*, Set, TransactionError, TransactionTrait};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info, instrument};
use uuid::Uuid;
use validator::Validate;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SplitCriteria {
    EvenSplit,
    // Add other variants as needed
}

#[derive(Debug, Serialize, Deserialize, Validate, Clone)]
pub struct SplitOrderCommand {
    pub order_id: Uuid,
    #[validate(length(min = 2, message = "At least 2 splits are required"))]
    pub splits: Vec<SplitInstruction>,
}

#[derive(Debug, Serialize, Deserialize, Validate, Clone)]
pub struct SplitInstruction {
    pub items: Vec<SplitItem>,
}

#[derive(Debug, Serialize, Deserialize, Validate, Clone)]
pub struct SplitItem {
    pub order_item_id: Uuid,
    #[validate(range(min = 1))]
    pub quantity: i32,
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

        self.log_and_trigger_events(&event_sender, &split_orders)
            .await?;

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
        let self_clone = self.clone();
        db.transaction::<_, Vec<order_entity::Model>, ServiceError>(move |txn| {
            Box::pin(async move {
                let original_order = self_clone.fetch_original_order(txn).await?;
                let _order_items = self_clone.fetch_order_items(txn).await?;
                
                // For now, we'll just create a simple split by creating a new order
                // with half the items (or based on the criteria)
                let mut split_orders = Vec::new();
                
                // Keep the original order
                split_orders.push(original_order.clone());
                
                // Create a new order (simplified implementation)
                let new_order_number = format!("{}-SPLIT", original_order.order_number);
                let new_order = order_entity::ActiveModel {
                    id: Set(Uuid::new_v4()),
                    order_number: Set(new_order_number),
                    status: Set(original_order.status.clone()),
                    customer_email: Set(original_order.customer_email.clone()),
                    customer_id: Set(original_order.customer_id),
                    total_amount: Set(original_order.total_amount / 2.0), // Simple split
                    shipping_address: Set(original_order.shipping_address.clone()),
                    billing_address: Set(original_order.billing_address.clone()),
                    payment_method: Set(original_order.payment_method.clone()),
                    shipping_method: Set(original_order.shipping_method.clone()),
                    tracking_number: Set(None),
                    notes: Set(Some(format!("Split from order {}", original_order.id))),
                    created_at: Set(Utc::now()),
                    updated_at: Set(Utc::now()),
                    version: Set(1),
                    ..Default::default()
                };
                
                let new_order = new_order.insert(txn).await
                    .map_err(|e| ServiceError::db_error(e))?;
                
                split_orders.push(new_order);
                
                Ok(split_orders)
            })
        })
        .await
        .map_err(|e| {
            error!("Transaction failed for order split: {}", e);
            match e {
                TransactionError::Connection(db_err) => ServiceError::db_error(db_err),
                TransactionError::Transaction(service_err) => service_err,
            }
        })
    }

    async fn fetch_original_order(
        &self,
        txn: &DatabaseTransaction,
    ) -> Result<order_entity::Model, ServiceError> {
        Order::find_by_id(self.order_id)
            .one(txn)
            .await
            .map_err(|e| {
                error!("Failed to find order {}: {}", self.order_id, e);
                ServiceError::db_error(e)
            })?
            .ok_or_else(|| {
                error!("Order {} not found", self.order_id);
                ServiceError::NotFound(format!("Order {} not found", self.order_id))
            })
    }

    async fn fetch_order_items(
        &self,
        txn: &DatabaseTransaction,
    ) -> Result<Vec<order_item_entity::Model>, ServiceError> {
        OrderItem::find()
            .filter(order_item_entity::Column::OrderId.eq(self.order_id))
            .all(txn)
            .await
            .map_err(|e| {
                error!("Failed to fetch order items for order {}: {}", self.order_id, e);
                ServiceError::db_error(e)
            })
    }

    fn apply_split_criteria(
        &self,
        items: &[order_item_entity::Model],
    ) -> (Vec<order_item_entity::Model>, Vec<order_item_entity::Model>) {
        match self.split_criteria {
            SplitCriteria::EvenSplit => {
                let split_point = items.len() / 2;
                let items_for_new_order = items[..split_point].to_vec();
                let remaining_items = items[split_point..].to_vec();
                (items_for_new_order, remaining_items)
            }
            // Implement other split criteria as needed
        }
    }

    async fn create_new_order(
        &self,
        txn: &DatabaseTransaction,
        original_order: &order_entity::Model,
    ) -> Result<order_entity::Model, ServiceError> {
        let new_order = order_entity::ActiveModel {
            customer_id: Set(original_order.customer_id),
            status: Set(OrderStatus::Pending),
            created_at: Set(Utc::now()),
            updated_at: Set(Utc::now()),
            ..Default::default()
        };

        new_order.insert(txn).await.map_err(|e| {
            error!("Failed to create new order: {}", e);
            ServiceError::db_error(e)
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
                error!("Failed to update item for new order: {}", e);
                ServiceError::db_error(e)
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
            error!("Failed to update original order: {}", e);
            ServiceError::db_error(e)
        })
    }

    async fn log_and_trigger_events(
        &self,
        event_sender: &EventSender,
        split_orders: &[order_entity::Model],
    ) -> Result<(), ServiceError> {
        for order in split_orders {
            info!("Order ID {} split into new orders", order.id);
            event_sender
                .send(Event::OrderUpdated(order.id))
                .await
                .map_err(|e| {
                    error!(
                        "Failed to send OrderSplit event for order ID {}: {:?}",
                        order.id, e
                    );
                    ServiceError::EventError(e.to_string())
                })?;
        }
        Ok(())
    }

    async fn create_note(
        &self,
        txn: &DatabaseTransaction,
        order_id: Uuid,
        note: String,
    ) -> Result<(), ServiceError> {
        let new_note = order_note_entity::ActiveModel {
            order_id: Set(order_id),
            note: Set(note),
            created_at: Set(Utc::now().naive_utc()),
            ..Default::default()
        };

        new_note.insert(txn).await.map_err(|e| {
            error!("Failed to create order note: {}", e);
            ServiceError::db_error(e)
        })?;

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
            created_at: order.created_at,
        }
    }
}
