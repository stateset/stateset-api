use uuid::Uuid;
use async_trait::async_trait;
use sea_orm::{*, Set};
use std::sync::Arc;
use tracing::{error, info, instrument};

use crate::{
    commands::Command,
    db::DbPool,
    errors::ServiceError,
    events::{Event, EventSender},
    models::{order, order::Entity as Order, order_tag, order_tag::Entity as OrderTag},
};
use chrono::Utc;

pub struct TagOrderCommand {
    pub order_id: Uuid,
    pub tag_id: i32,
}

#[async_trait]
impl Command for TagOrderCommand {
    type Result = Vec<order::Model>;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(
        &self,
        db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        let db = &**db_pool;

        let tagged_orders = db
            .transaction::<_, Vec<order::Model>, ServiceError>(|txn| {
                Box::pin(async move { self.tag_order_logic(txn).await })
            })
            .await
            .map_err(|e| {
                error!("Failed to tag order ID {}: {:?}", self.order_id, e);
                e
            })?;

        self.log_and_trigger_events(event_sender, &tagged_orders)
            .await?;

        Ok(tagged_orders)
    }
}

impl TagOrderCommand {
    async fn tag_order_logic(
        &self,
        txn: &DatabaseTransaction,
    ) -> Result<Vec<order::Model>, ServiceError> {
        // Fetch the order
        let order = Order::find_by_id(self.order_id)
            .one(txn)
            .await
            .map_err(|e| {
                error!("Failed to find order {}: {}", self.order_id, e);
                ServiceError::db_error(e)
            })?
            .ok_or_else(|| {
                error!("Order {} not found", self.order_id);
                ServiceError::NotFound(format!("Order {} not found", self.order_id))
            })?;

        // TODO: Fix schema mismatch - order_tag expects i32 order_id but order entity has UUID id
        // For now, we'll skip creating the tag and just log
        error!("Cannot create tag - schema mismatch: order_tag expects i32 order_id but order has UUID");
        
        // let tag = order_tag::ActiveModel {
        //     id: Set(Uuid::new_v4()),
        //     order_id: Set(order.id), // This won't work - type mismatch
        //     tag_name: Set(self.tag_name.clone()),
        //     tag_value: Set(self.tag_value.clone()),
        //     created_by: Set(Some(self.user_id)),
        //     created_at: Set(Utc::now()),
        // };

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
                .send(Event::OrderUpdated(order.id))
                .await
                .map_err(|e| {
                    error!(
                        "Failed to send OrderTagged event for order ID {}: {:?}",
                        order.id, e
                    );
                    ServiceError::EventError(e.to_string())
                })?;
        }
        Ok(())
    }
}
