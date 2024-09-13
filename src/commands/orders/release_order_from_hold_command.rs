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
    static ref ORDER_RELEASES_FROM_HOLD: IntCounter = 
        IntCounter::new("order_releases_from_hold_total", "Total number of orders released from hold")
            .expect("metric can be created");

    static ref ORDER_RELEASES_FROM_HOLD_FAILURES: IntCounter = 
        IntCounter::new("order_releases_from_hold_failures_total", "Total number of failed order releases from hold")
            .expect("metric can be created");
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct ReleaseOrderFromHoldCommand {
    pub order_id: Uuid,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ReleaseOrderFromHoldResult {
    pub id: Uuid,
    pub status: String,
    pub released_at: DateTime<Utc>,
}

#[async_trait::async_trait]
impl Command for ReleaseOrderFromHoldCommand {
    type Result = ReleaseOrderFromHoldResult;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(
        &self,
        db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        let db = db_pool.as_ref();

        let updated_order = self.release_order_from_hold(db).await?;

        self.log_and_trigger_event(&event_sender, &updated_order).await?;

        ORDER_RELEASES_FROM_HOLD.inc();

        Ok(ReleaseOrderFromHoldResult {
            id: updated_order.id,
            status: updated_order.status,
            released_at: updated_order.updated_at.and_utc(),
        })
    }
}

impl ReleaseOrderFromHoldCommand {
    async fn release_order_from_hold(
        &self,
        db: &DatabaseConnection,
    ) -> Result<order_entity::Model, ServiceError> {
        let order = Order::find_by_id(self.order_id)
            .one(db)
            .await
            .map_err(|e| {
                ORDER_RELEASES_FROM_HOLD_FAILURES.inc();
                let msg = format!("Failed to find order with ID {}: {}", self.order_id, e);
                error!("{}", msg);
                ServiceError::DatabaseError(msg)
            })?
            .ok_or_else(|| {
                ORDER_RELEASES_FROM_HOLD_FAILURES.inc();
                let msg = format!("Order with ID {} not found", self.order_id);
                error!("{}", msg);
                ServiceError::NotFound(msg)
            })?;

        if order.status != OrderStatus::OnHold.to_string() {
            ORDER_RELEASES_FROM_HOLD_FAILURES.inc();
            let msg = format!("Order with ID {} is not on hold", self.order_id);
            error!("{}", msg);
            return Err(ServiceError::InvalidOperation(msg));
        }

        let mut order: order_entity::ActiveModel = order.into();
        order.status = Set(OrderStatus::Pending.to_string());
        order.updated_at = Set(Utc::now().naive_utc());

        order.update(db).await.map_err(|e| {
            ORDER_RELEASES_FROM_HOLD_FAILURES.inc();
            let msg = format!("Failed to update order status to Pending for order ID {}: {}", self.order_id, e);
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
            "Order released from hold successfully"
        );

        event_sender
            .send(Event::OrderReleasedFromHold(self.order_id))
            .await
            .map_err(|e| {
                ORDER_RELEASES_FROM_HOLD_FAILURES.inc();
                let msg = format!("Failed to send event for order released from hold: {}", e);
                error!("{}", msg);
                ServiceError::EventError(msg)
            })
    }
}