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
    static ref ORDER_RELEASES_FROM_HOLD: IntCounter = IntCounter::new(
        "order_releases_from_hold_total",
        "Total number of orders released from hold"
    )
    .expect("metric can be created");
    static ref ORDER_RELEASES_FROM_HOLD_FAILURES: IntCounter = IntCounter::new(
        "order_releases_from_hold_failures_total",
        "Total number of failed order releases from hold"
    )
    .expect("metric can be created");
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct ReleaseOrderFromHoldCommand {
    pub order_id: Uuid,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ReleaseOrderFromHoldResult {
    pub id: Uuid,
    pub order_number: String,
    pub status: String,
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

        self.log_and_trigger_event(&event_sender, &updated_order)
            .await?;

        ORDER_RELEASES_FROM_HOLD.inc();

        Ok(ReleaseOrderFromHoldResult {
            id: updated_order.id,
            order_number: updated_order.order_number,
            status: updated_order.status.to_string(),
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
                error!("Failed to find order: {}", e);
                ServiceError::db_error(e)
            })?
            .ok_or_else(|| {
                let msg = format!("Order {} not found", self.order_id);
                error!("{}", msg);
                ServiceError::NotFound(msg)
            })?;

        // Check if order is currently on hold
        if order.status != OrderStatus::OnHold {
            let msg = format!("Order {} is not on hold", self.order_id);
            error!("{}", msg);
            return Err(ServiceError::InvalidOperation(msg));
        }

        // Update order status
        let mut order: order_entity::ActiveModel = order.into();
        order.status = Set(OrderStatus::Pending);
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
        updated_order: &order_entity::Model,
    ) -> Result<(), ServiceError> {
        info!(
            order_id = %self.order_id,
            "Order released from hold successfully"
        );

        event_sender
            .send(Event::OrderUpdated {
                order_id: self.order_id,
                checkout_session_id: None, // Orders may not have checkout_session_id
                status: Some(updated_order.status.clone()),
                refunds: vec![],
            })
            .await
            .map_err(|e| {
                ORDER_RELEASES_FROM_HOLD_FAILURES.inc();
                let msg = format!("Failed to send event for order released from hold: {}", e);
                error!("{}", msg);
                ServiceError::EventError(msg)
            })
    }
}
