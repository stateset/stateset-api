use async_trait::async_trait;;
use serde::{Serialize, Deserialize};
use std::sync::Arc;
use sea_orm::*;
use crate::{errors::ServiceError, db::DbPool, models::{order_entity, order_entity::Entity as Order, OrderStatus}};
use crate::events::{Event, EventSender};
use tracing::{info, error, instrument};
use prometheus::IntCounter;

lazy_static! {
    static ref ORDER_RELEASES_FROM_HOLD: IntCounter = 
        IntCounter::new("order_releases_from_hold_total", "Total number of orders released from hold")
            .expect("metric can be created");

    static ref ORDER_RELEASES_FROM_HOLD_FAILURES: IntCounter = 
        IntCounter::new("order_releases_from_hold_failures_total", "Total number of failed order releases from hold")
            .expect("metric can be created");
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ReleaseOrderFromHoldCommand {
    pub order_id: i32,
}

#[async_trait]
impl Command for ReleaseOrderFromHoldCommand {
    type Result = order_entity::Model;

    #[instrument(skip(db_pool, event_sender))]
    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        let db = db_pool.get().map_err(|e| {
            ORDER_RELEASES_FROM_HOLD_FAILURES.inc();
            error!("Failed to get database connection: {}", e);
            ServiceError::DatabaseError
        })?;

        let order = Order::find_by_id(self.order_id)
            .one(&db)
            .await
            .map_err(|e| {
                ORDER_RELEASES_FROM_HOLD_FAILURES.inc();
                error!("Failed to find order with ID {}: {}", self.order_id, e);
                ServiceError::DatabaseError
            })?
            .ok_or_else(|| {
                ORDER_RELEASES_FROM_HOLD_FAILURES.inc();
                error!("Order with ID {} not found", self.order_id);
                ServiceError::NotFound
            })?;

        let mut order: order_entity::ActiveModel = order.into();
        order.status = Set(OrderStatus::Pending.to_string());

        let updated_order = order.update(&db).await.map_err(|e| {
            ORDER_RELEASES_FROM_HOLD_FAILURES.inc();
            error!("Failed to update order status to Pending for order ID {}: {}", self.order_id, e);
            ServiceError::DatabaseError
        })?;

        // Trigger an event
        if let Err(e) = event_sender.send(Event::OrderReleasedFromHold(self.order_id)).await {
            ORDER_RELEASES_FROM_HOLD_FAILURES.inc();
            error!("Failed to send OrderReleasedFromHold event for order ID {}: {}", self.order_id, e);
            return Err(ServiceError::EventError(e.to_string()));
        }

        ORDER_RELEASES_FROM_HOLD.inc();

        info!(
            order_id = %self.order_id,
            "Order released from hold successfully"
        );

        Ok(updated_order)
    }
}