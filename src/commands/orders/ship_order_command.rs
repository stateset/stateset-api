use crate::{
    commands::Command,
    db::DbPool,
    errors::ServiceError,
    events::{Event, EventSender},
    models::{
        order::OrderStatus,
        order_entity::{self, Entity as Order},
    },
};
use chrono::{DateTime, Utc};
use lazy_static::lazy_static;
use prometheus::IntCounter;
use sea_orm::{*, Set};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info, instrument};
use uuid::Uuid;
use validator::Validate;

lazy_static! {
    static ref ORDERS_SHIPPED: IntCounter =
        IntCounter::new("orders_shipped_total", "Total number of orders shipped")
            .expect("metric can be created");
    static ref ORDER_SHIP_FAILURES: IntCounter = IntCounter::new(
        "order_ship_failures_total",
        "Total number of failed order shipments"
    )
    .expect("metric can be created");
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct ShipOrderCommand {
    pub order_id: Uuid,
    pub user_id: Uuid,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ShipOrderResult {
    pub id: Uuid,
    pub status: String,
    pub shipped_at: DateTime<Utc>,
    pub shipped_by: Uuid,
}

#[async_trait::async_trait]
impl Command for ShipOrderCommand {
    type Result = ShipOrderResult;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(
        &self,
        db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        let db = db_pool.as_ref();

        let shipped_order = self.ship_order(db).await?;

        self.log_and_trigger_event(&event_sender, &shipped_order)
            .await?;

        ORDERS_SHIPPED.inc();

        Ok(ShipOrderResult {
            id: shipped_order.id,
            status: shipped_order.status,
            shipped_at: shipped_order.updated_at,
            shipped_by: self.user_id,
        })
    }
}

impl ShipOrderCommand {
    async fn ship_order(
        &self,
        db: &DatabaseConnection,
    ) -> Result<order_entity::Model, ServiceError> {
        let order = Order::find_by_id(self.order_id)
            .one(db)
            .await
            .map_err(|e| {
                error!("Failed to find order: {}", e);
                ServiceError::db_error(e)
            })?
            .ok_or_else(|| {
                let msg = format!("Order {} not found", self.order_id);
                error!("{}", msg);
                ServiceError::NotFound(msg)
            })?;

        // Check if order is already shipped
        if order.status == OrderStatus::Shipped {
            let msg = format!("Order {} is already shipped", self.order_id);
            error!("{}", msg);
            return Err(ServiceError::InvalidOperation(msg));
        }

        // Update order status
        let mut order: order_entity::ActiveModel = order.into();
        order.status = Set(OrderStatus::Shipped);
        order.updated_at = Set(Utc::now());

        order.update(db).await.map_err(|e| {
            let msg = format!("Failed to update order status: {}", e);
            error!("{}", msg);
            ServiceError::db_error(e)
        })
    }

    async fn log_and_trigger_event(
        &self,
        event_sender: &EventSender,
        _shipped_order: &order_entity::Model,
    ) -> Result<(), ServiceError> {
        info!(
            order_id = %self.order_id,
            user_id = %self.user_id,
            "Order successfully shipped"
        );

        event_sender
            .send(Event::OrderUpdated {
                order_id: self.order_id,
                checkout_session_id: None,
                status: None,
                refunds: vec![],
            })
            .await
            .map_err(|e| {
                ORDER_SHIP_FAILURES.inc();
                let msg = format!("Failed to send event for shipped order: {}", e);
                error!("{}", msg);
                ServiceError::EventError(msg)
            })
    }
}
