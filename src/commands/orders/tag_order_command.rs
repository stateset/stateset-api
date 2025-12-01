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
    pub tag_name: String,
    pub tag_value: Option<String>,
    pub created_by: Option<Uuid>,
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

        // Create the order tag
        let tag = order_tag::ActiveModel {
            id: Set(Uuid::new_v4()),
            order_id: Set(order.id),
            tag_name: Set(self.tag_name.clone()),
            tag_value: Set(self.tag_value.clone()),
            created_by: Set(self.created_by),
            created_at: Set(Utc::now()),
        };

        // Insert the tag
        tag.insert(txn).await.map_err(|e| {
            error!("Failed to create tag for order {}: {}", self.order_id, e);
            ServiceError::db_error(e)
        })?;

        info!("Successfully tagged order {} with '{}'", order.id, self.tag_name);

        Ok(vec![order])
    }

    async fn log_and_trigger_events(
        &self,
        event_sender: Arc<EventSender>,
        tagged_orders: &[order::Model],
    ) -> Result<(), ServiceError> {
        for order in tagged_orders {
            info!("Order ID {} tagged with '{}'", order.id, self.tag_name);
            event_sender
                .send(Event::OrderUpdated {
                order_id: order.id,
                checkout_session_id: None,
                status: None,
                refunds: vec![],
            })
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
