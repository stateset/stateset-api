use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info, instrument};
use validator::Validate;

use crate::{
    db::DbPool,
    errors::ServiceError,
    events::{Event, EventSender},
    models::{Order, OrderItem},
};
use diesel::prelude::*;
use prometheus::IntCounter;
use lazy_static::lazy_static;

lazy_static! {
    static ref ORDER_ITEM_UPDATES: IntCounter = 
        IntCounter::new("order_item_updates_total", "Total number of order item updates")
            .expect("metric can be created");

    static ref ORDER_ITEM_UPDATE_FAILURES: IntCounter = 
        IntCounter::new("order_item_update_failures_total", "Total number of failed order item updates")
            .expect("metric can be created");
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct UpdateOrderItemsCommand {
    pub order_id: i32,
    #[validate(length(min = 1))]
    pub items: Vec<OrderItem>,
}

#[async_trait]
impl Command for UpdateOrderItemsCommand {
    type Result = Order;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|e| {
            ORDER_ITEM_UPDATE_FAILURES.inc();
            error!("Failed to get database connection: {}", e);
            ServiceError::DatabaseError
        })?;

        let updated_order = conn.transaction(|| {
            self.delete_existing_items(&conn)?;
            self.insert_new_items(&conn)?;
            self.recalculate_order_total(&conn)
        }).map_err(|e: ServiceError| {
            error!("Transaction failed for updating order items in order ID {}: {}", self.order_id, e);
            ORDER_ITEM_UPDATE_FAILURES.inc();
            e
        })?;

        self.log_and_trigger_event(event_sender, &updated_order).await?;

        ORDER_ITEM_UPDATES.inc();
        info!(order_id = %self.order_id, "Order items updated successfully");

        Ok(updated_order)
    }
}

impl UpdateOrderItemsCommand {
    fn delete_existing_items(&self, conn: &PgConnection) -> Result<(), ServiceError> {
        diesel::delete(order_items::table.filter(order_items::order_id.eq(self.order_id)))
            .execute(conn)
            .map_err(|e| {
                error!("Failed to delete order items for order ID {}: {}", self.order_id, e);
                ORDER_ITEM_UPDATE_FAILURES.inc();
                ServiceError::DatabaseError
            })?;
        Ok(())
    }

    fn insert_new_items(&self, conn: &PgConnection) -> Result<(), ServiceError> {
        for item in &self.items {
            diesel::insert_into(order_items::table)
                .values(item)
                .execute(conn)
                .map_err(|e| {
                    error!("Failed to insert order item for order ID {}: {}", self.order_id, e);
                    ORDER_ITEM_UPDATE_FAILURES.inc();
                    ServiceError::DatabaseError
                })?;
        }
        Ok(())
    }

    fn recalculate_order_total(&self, conn: &PgConnection) -> Result<Order, ServiceError> {
        // Implement the logic to recalculate the order total
        orders::table.find(self.order_id)
            .first::<Order>(conn)
            .map_err(|e| {
                error!("Failed to recalculate total for order ID {}: {}", self.order_id, e);
                ServiceError::DatabaseError
            })
    }

    async fn log_and_trigger_event(&self, event_sender: Arc<EventSender>, updated_order: &Order) -> Result<(), ServiceError> {
        if let Err(e) = event_sender.send(Event::OrderUpdated(self.order_id)).await {
            ORDER_ITEM_UPDATE_FAILURES.inc();
            error!("Failed to send OrderUpdated event for order ID {}: {}", self.order_id, e);
            return Err(ServiceError::EventError(e.to_string()));
        }
        Ok(())
    }
}
