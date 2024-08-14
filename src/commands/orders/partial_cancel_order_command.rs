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

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct PartialCancelOrderCommand {
    pub order_id: i32,

    #[validate(length(min = 1))]
    pub item_ids: Vec<i32>, // IDs of items to cancel
}

#[async_trait]
impl Command for PartialCancelOrderCommand {
    type Result = Order;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|e| {
            error!("Failed to get database connection: {:?}", e);
            ServiceError::DatabaseError
        })?;

        let updated_order = conn.transaction::<Order, ServiceError, _>(|| {
            self.remove_items(&conn)?;
            self.recalculate_order_total(&conn)
        }).map_err(|e| {
            error!("Failed to partially cancel order ID {}: {:?}", self.order_id, e);
            e
        })?;

        self.log_and_trigger_event(event_sender, &updated_order).await?;

        Ok(updated_order)
    }
}

impl PartialCancelOrderCommand {
    fn remove_items(&self, conn: &PgConnection) -> Result<(), ServiceError> {
        diesel::delete(order_items::table.filter(order_items::id.eq_any(&self.item_ids)))
            .execute(conn)
            .map_err(|e| {
                error!("Failed to remove items from order ID {}: {:?}", self.order_id, e);
                ServiceError::DatabaseError
            })?;
        Ok(())
    }

    fn recalculate_order_total(&self, conn: &PgConnection) -> Result<Order, ServiceError> {
        // Implement the logic to recalculate the order total
        // Placeholder: fetch the updated order from the database
        orders::table.find(self.order_id)
            .first::<Order>(conn)
            .map_err(|e| {
                error!("Failed to recalculate total for order ID {}: {:?}", self.order_id, e);
                ServiceError::DatabaseError
            })
    }

    async fn log_and_trigger_event(
        &self,
        event_sender: Arc<EventSender>,
        updated_order: &Order,
    ) -> Result<(), ServiceError> {
        info!("Partial cancellation of items for order ID: {}", self.order_id);

        event_sender
            .send(Event::OrderUpdated(self.order_id))
            .await
            .map_err(|e| {
                error!("Failed to send OrderUpdated event for order ID {}: {:?}", self.order_id, e);
                ServiceError::EventError(e.to_string())
            })
    }
}
