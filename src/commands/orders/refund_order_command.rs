use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info, instrument};

use crate::{
    db::DbPool,
    errors::ServiceError,
    events::{Event, EventSender},
    models::{Order, OrderNote, NewOrderNote},
};
use diesel::prelude::*;

pub struct RefundOrderCommand {
    pub order_id: i32,
    pub refund_amount: f64,
    pub reason: String,
}

#[async_trait]
impl Command for RefundOrderCommand {
    type Result = Order;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|e| {
            error!("Failed to get database connection: {:?}", e);
            ServiceError::DatabaseError
        })?;

        let updated_order = conn.transaction::<Order, ServiceError, _>(|| {
            self.process_refund(&conn)?;
            self.log_refund_reason(&conn)
        }).map_err(|e| {
            error!("Failed to process refund for order ID {}: {:?}", self.order_id, e);
            e
        })?;

        self.log_and_trigger_event(event_sender, &updated_order).await?;

        Ok(updated_order)
    }
}

impl RefundOrderCommand {
    fn process_refund(&self, conn: &PgConnection) -> Result<Order, ServiceError> {
        diesel::update(orders::table.find(self.order_id))
            .set(orders::total_amount.eq(orders::total_amount - self.refund_amount))
            .get_result::<Order>(conn)
            .map_err(|e| {
                error!("Failed to update order amount for refund: {:?}", e);
                ServiceError::DatabaseError
            })
    }

    fn log_refund_reason(&self, conn: &PgConnection) -> Result<(), ServiceError> {
        diesel::insert_into(order_notes::table)
            .values(&NewOrderNote {
                order_id: self.order_id,
                note: format!("Refunded: {} - Reason: {}", self.refund_amount, self.reason),
            })
            .execute(conn)
            .map_err(|e| {
                error!("Failed to log refund reason for order ID {}: {:?}", self.order_id, e);
                ServiceError::DatabaseError
            })?;
        Ok(())
    }

    async fn log_and_trigger_event(
        &self,
        event_sender: Arc<EventSender>,
        updated_order: &Order,
    ) -> Result<(), ServiceError> {
        info!("Order ID {} refunded with amount: {}", self.order_id, self.refund_amount);

        event_sender
            .send(Event::OrderRefunded(self.order_id))
            .await
            .map_err(|e| {
                error!("Failed to send OrderRefunded event for order ID {}: {:?}", self.order_id, e);
                ServiceError::EventError(e.to_string())
            })
    }
}
