use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info, instrument};

use crate::{
    db::DbPool,
    errors::ServiceError,
    events::{Event, EventSender},
    models::{Order, OrderItem, OrderStatus},
};
use diesel::prelude::*;
use chrono::{DateTime, Utc};

pub struct TagOrderCommand {
    pub order_id: i32,
    pub tag_id: i32,
}

#[async_trait]
impl Command for TagOrderCommand {
    type Result = Vec<Order>;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|e| {
            error!("Failed to get database connection: {:?}", e);
            ServiceError::DatabaseError
        })?;

        let split_orders = conn.transaction::<Vec<Order>, ServiceError, _>(|| {
            self.tag_order_logic(&conn)
        }).map_err(|e| {
            error!("Failed to tag order ID {}: {:?}", self.order_id, e);
            e
        })?;

        self.log_and_trigger_events(event_sender, &split_orders).await?;

        Ok(split_orders)
    }
}

impl TagOrderCommand {
    fn tag_order_logic(&self, conn: &PgConnection) -> Result<Vec<Order>, ServiceError> {
        // Placeholder for actual tag logic
        // The actual implementation should tag the order based on the tag_id
        Ok(vec![]) // Return the list of tagged orders
    }

    async fn log_and_trigger_events(
        &self,
        event_sender: Arc<EventSender>,
        split_orders: &[Order],
    ) -> Result<(), ServiceError> {
        for order in split_orders {
            info!("Order ID {} split into new order ID {}", self.order_id, order.id);
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