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

pub struct SplitOrderCommand {
    pub order_id: i32,
    pub split_criteria: SplitCriteria,
}

#[async_trait]
impl Command for SplitOrderCommand {
    type Result = Vec<Order>;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|e| {
            error!("Failed to get database connection: {:?}", e);
            ServiceError::DatabaseError
        })?;

        let split_orders = conn.transaction::<Vec<Order>, ServiceError, _>(|| {
            self.split_order_logic(&conn)
        }).map_err(|e| {
            error!("Failed to split order ID {}: {:?}", self.order_id, e);
            e
        })?;

        self.log_and_trigger_events(event_sender, &split_orders).await?;

        Ok(split_orders)
    }
}

impl SplitOrderCommand {
    fn split_order_logic(&self, conn: &PgConnection) -> Result<Vec<Order>, ServiceError> {
        // Placeholder for actual split logic
        // The actual implementation should split the order based on the split_criteria
        Ok(vec![]) // Return the list of split orders
    }

    async fn log_and_trigger_events(
        &self,
        event_sender: Arc<EventSender>,
        split_orders: &[Order],
    ) -> Result<(), ServiceError> {
        for order in split_orders {
            info!("Order ID {} split into new order ID {}", self.order_id, order.id);
            event_sender
                .send(Event::OrderSplit(order.id))
                .await
                .map_err(|e| {
                    error!("Failed to send OrderSplit event for order ID {}: {:?}", order.id, e);
                    ServiceError::EventError(e.to_string())
                })?;
        }
        Ok(())
    }
}