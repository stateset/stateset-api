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
    static ref ORDER_STATUS_UPDATES: IntCounter =
        IntCounter::new("order_status_updates_total", "Total number of order status updates")
            .expect("metric can be created");

    static ref ORDER_STATUS_UPDATE_FAILURES: IntCounter =
        IntCounter::new("order_status_update_failures_total", "Total number of failed order status updates")
            .expect("metric can be created");
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct UpdateOrderStatusCommand {
    pub order_id: Uuid,
    pub new_status: OrderStatus,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateOrderStatusResult {
    pub id: Uuid,
    pub status: String,
    pub updated_at: DateTime<Utc>,
}

#[async_trait::async_trait]
impl Command for UpdateOrderStatusCommand {
    type Result = UpdateOrderStatusResult;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(
        &self,
        db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        let db = db_pool.as_ref();

        let updated_order = self.update_order_status(db).await?;

        self.log_and_trigger_event(&event_sender, &updated_order).await?;

        ORDER_STATUS_UPDATES.inc();

        Ok(UpdateOrderStatusResult {
            id: updated_order.id,
            status: updated_order.status,
            updated_at: updated_order.updated_at.and_utc(),
        })
    }
}

impl UpdateOrderStatusCommand {
    async fn update_order_status(
        &self,
        db: &DatabaseConnection,
    ) -> Result<order_entity::Model, ServiceError> {
        let order = Order::find_by_id(self.order_id)
            .one(db)
            .await
            .map_err(|e| {
                ORDER_STATUS_UPDATE_FAILURES.inc();
                let msg = format!("Failed to find order {}: {}", self.order_id, e);
                error!("{}", msg);
                ServiceError::DatabaseError(msg)
            })?
            .ok_or_else(|| {
                ORDER_STATUS_UPDATE_FAILURES.inc();
                let msg = format!("Order {} not found", self.order_id);
                error!("{}", msg);
                ServiceError::NotFound(msg)
            })?;

        let mut order: order_entity::ActiveModel = order.into();
        order.status = Set(self.new_status.to_string());
        order.updated_at = Set(Utc::now().naive_utc());

        order.update(db).await.map_err(|e| {
            ORDER_STATUS_UPDATE_FAILURES.inc();
            let msg = format!("Failed to update order status for order {}: {}", self.order_id, e);
            error!("{}", msg);
            ServiceError::DatabaseError(msg)
        })
    }

    async fn log_and_trigger_event(
        &self,
        event_sender: &EventSender,
        updated_order: &order_entity::Model,
    ) -> Result<(), ServiceError> {
        info!(
            order_id = %self.order_id,
            new_status = %self.new_status.to_string(),
            "Order status updated successfully"
        );

        event_sender
            .send(Event::OrderUpdated(self.order_id))
            .await
            .map_err(|e| {
                ORDER_STATUS_UPDATE_FAILURES.inc();
                let msg = format!("Failed to send event for updated order: {}", e);
                error!("{}", msg);
                ServiceError::EventError(msg)
            })
    }
}
