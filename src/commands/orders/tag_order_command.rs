use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info, instrument};
use sea_orm::*;

use crate::{
    db::DbPool,
    errors::ServiceError,
    events::{Event, EventSender},
    models::{order, order::Entity as Order, order_tag, order_tag::Entity as OrderTag},
};
use chrono::{DateTime, Utc};

pub struct TagOrderCommand {
    pub order_id: i32,
    pub tag_id: i32,
}

#[async_trait]
impl Command for TagOrderCommand {
    type Result = Vec<order::Model>;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        let db = db_pool.get().map_err(|e| {
            error!("Failed to get database connection: {:?}", e);
            ServiceError::DatabaseError
        })?;

        let tagged_orders = db.transaction::<_, Vec<order::Model>, ServiceError>(|txn| {
            Box::pin(async move {
                self.tag_order_logic(txn).await
            })
        }).await.map_err(|e| {
            error!("Failed to tag order ID {}: {:?}", self.order_id, e);
            e
        })?;

        self.log_and_trigger_events(event_sender, &tagged_orders).await?;

        Ok(tagged_orders)
    }
}

impl TagOrderCommand {
    async fn tag_order_logic(&self, txn: &DatabaseTransaction) -> Result<Vec<order::Model>, ServiceError> {
        // Fetch the order
        let order = Order::find_by_id(self.order_id)
            .one(txn)
            .await
            .map_err(|e| {
                error!("Failed to fetch order {}: {:?}", self.order_id, e);
                ServiceError::DatabaseError
            })?
            .ok_or_else(|| {
                error!("Order {} not found", self.order_id);
                ServiceError::NotFound
            })?;

        // Create a new order tag
        let new_tag = order_tag::ActiveModel {
            order_id: Set(self.order_id),
            tag_id: Set(self.tag_id),
            created_at: Set(Utc::now().naive_utc()),
            ..Default::default()
        };

        new_tag.insert(txn).await.map_err(|e| {
            error!("Failed to insert new tag for order {}: {:?}", self.order_id, e);
            ServiceError::DatabaseError
        })?;

        // You might want to update the order here if needed
        // For example, if you want to store the latest tag directly on the order:
        // let mut order: order_entity::ActiveModel = order.into();
        // order.latest_tag_id = Set(Some(self.tag_id));
        // order.update(txn).await.map_err(|e| {
        //     error!("Failed to update order {}: {:?}", self.order_id, e);
        //     ServiceError::DatabaseError
        // })?;

        Ok(vec![order])
    }

    async fn log_and_trigger_events(
        &self,
        event_sender: Arc<EventSender>,
        tagged_orders: &[order::Model],
    ) -> Result<(), ServiceError> {
        for order in tagged_orders {
            info!("Order ID {} tagged with tag ID {}", order.id, self.tag_id);
            event_sender
                .send(Event::OrderTagged(order.id))
                .await
                .map_err(|e| {
                    error!("Failed to send OrderTagged event for order ID {}: {:?}", order.id, e);
                    ServiceError::EventError(e.to_string())
                })?;
        }
        Ok(())
    }
}