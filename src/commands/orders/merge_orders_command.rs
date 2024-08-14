use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info, instrument};
use validator::Validate;

use crate::{
    db::DbPool,
    errors::ServiceError,
    events::{Event, EventSender},
    models::{Order, OrderItem, OrderStatus},
};
use diesel::prelude::*;
use chrono::{DateTime, Utc};

pub struct MergeOrdersCommand {
    pub order_ids: Vec<i32>,
}

#[async_trait]
impl Command for MergeOrdersCommand {
    type Result = Order;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|e| {
            error!("Failed to get database connection: {:?}", e);
            ServiceError::DatabaseError
        })?;

        let merged_order = conn.transaction::<Order, ServiceError, _>(|| {
            self.merge_orders(&conn)
        }).map_err(|e| {
            error!("Failed to merge orders {:?}: {:?}", self.order_ids, e);
            e
        })?;

        self.log_and_trigger_event(event_sender, &merged_order).await?;

        Ok(merged_order)
    }
}

impl MergeOrdersCommand {
    fn merge_orders(&self, conn: &PgConnection) -> Result<Order, ServiceError> {
        // Implement the actual merge logic here
        // Placeholder: This should involve combining the items, handling any conflicts, and updating the database accordingly.
        Ok(Order::default()) // Return the merged order
    }

    async fn log_and_trigger_event(
        &self,
        event_sender: Arc<EventSender>,
        merged_order: &Order,
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
