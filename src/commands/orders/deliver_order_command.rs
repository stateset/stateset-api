use std::sync::Arc;
use sea_orm::*;
use crate::{
    db::DbPool,
    errors::ServiceError,
    events::{Event, EventSender},
    models::{
        order_entity::{self, Entity as Order},
        OrderStatus,
    },
};
use serde::{Deserialize, Serialize};
use tracing::{error, info, instrument};
use uuid::Uuid;
use validator::Validate;
use prometheus::IntCounter;
use lazy_static::lazy_static;
use chrono::{DateTime, Utc};

lazy_static! {
    static ref ORDERS_DELIVERED: IntCounter =
        IntCounter::new("orders_delivered_total", "Total number of orders delivered")
            .expect("metric can be created");

    static ref ORDER_DELIVER_FAILURES: IntCounter =
        IntCounter::new("order_deliver_failures_total", "Total number of failed order deliveries")
            .expect("metric can be created");
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct DeliverOrderCommand {
    pub order_id: Uuid,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeliverOrderResult {
    pub id: Uuid,
    pub status: String,
    pub delivered_at: DateTime<Utc>,
}

#[async_trait::async_trait]
impl Command for DeliverOrderCommand {
    type Result = DeliverOrderResult;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(
        &self,
        db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        let db = db_pool.as_ref();

        let delivered_order = self.deliver_order(db).await?;

        self.log_and_trigger_event(&event_sender, &delivered_order).await?;

        ORDERS_DELIVERED.inc();

        Ok(DeliverOrderResult {
            id: delivered_order.id,
            status: delivered_order.status,
            delivered_at: delivered_order.updated_at.and_utc(),
        })
    }
}

impl DeliverOrderCommand {
    async fn deliver_order(
        &self,
        db: &DatabaseConnection,
    ) -> Result<order_entity::Model, ServiceError> {
        let order = Order::find_by_id(self.order_id)
            .one(db)
            .await
            .map_err(|e| {
                ORDER_DELIVER_FAILURES.inc();
                let msg = format!("Failed to find order {}: {}", self.order_id, e);
                error!("{}", msg);
                ServiceError::DatabaseError(msg)
            })?
            .ok_or_else(|| {
                ORDER_DELIVER_FAILURES.inc();
                let msg = format!("Order {} not found", self.order_id);
                error!("{}", msg);
                ServiceError::NotFound(msg)
            })?;

        if order.status != OrderStatus::Shipped.to_string() {
            ORDER_DELIVER_FAILURES.inc();
            let msg = format!("Order {} is not shipped", self.order_id);
            error!("{}", msg);
            return Err(ServiceError::InvalidOperation(msg));
        }

        let mut order: order_entity::ActiveModel = order.into();
        order.status = Set(OrderStatus::Delivered.to_string());
        order.updated_at = Set(Utc::now().naive_utc());

        order.update(db).await.map_err(|e| {
            ORDER_DELIVER_FAILURES.inc();
            let msg = format!("Failed to update order status to 'Delivered' for order {}: {}", self.order_id, e);
            error!("{}", msg);
            ServiceError::DatabaseError(msg)
        })
    }

    async fn log_and_trigger_event(
        &self,
        event_sender: &EventSender,
        delivered_order: &order_entity::Model,
    ) -> Result<(), ServiceError> {
        info!(order_id = %self.order_id, "Order delivered successfully");

        event_sender
            .send(Event::OrderCompleted(self.order_id))
            .await
            .map_err(|e| {
                ORDER_DELIVER_FAILURES.inc();
                let msg = format!("Failed to send event for delivered order: {}", e);
                error!("{}", msg);
                ServiceError::EventError(msg)
            })
    }
}
