use crate::{
    commands::Command,
    db::DbPool,
    errors::ServiceError,
    events::{Event, EventSender},
    models::{
        order_entity::{self, Entity as Order},
        OrderStatus,
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
    static ref ORDERS_CONFIRMED: IntCounter =
        IntCounter::new("orders_confirmed_total", "Total number of orders confirmed")
            .expect("metric can be created");
    static ref ORDER_CONFIRM_FAILURES: IntCounter = IntCounter::new(
        "order_confirm_failures_total",
        "Total number of failed order confirmations"
    )
    .expect("metric can be created");
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct ConfirmOrderCommand {
    pub order_id: Uuid,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ConfirmOrderResult {
    pub id: Uuid,
    pub status: String,
    pub confirmed_at: DateTime<Utc>,
}

#[async_trait::async_trait]
impl Command for ConfirmOrderCommand {
    type Result = ConfirmOrderResult;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(
        &self,
        db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        let db = db_pool.as_ref();

        let confirmed_order = self.confirm_order(db).await?;

        self.log_and_trigger_event(&event_sender, &confirmed_order)
            .await?;

        ORDERS_CONFIRMED.inc();

        Ok(ConfirmOrderResult {
            id: confirmed_order.id,
            status: confirmed_order.status,
            confirmed_at: confirmed_order.updated_at,
        })
    }
}

impl ConfirmOrderCommand {
    async fn confirm_order(
        &self,
        db: &DatabaseConnection,
    ) -> Result<order_entity::Model, ServiceError> {
        let order = order_entity::Entity::find_by_id(self.order_id)
            .one(db)
            .await
            .map_err(|e| ServiceError::db_error(e))?
            .ok_or_else(|| {
                let msg = format!("Order {} not found", self.order_id);
                error!("{}", msg);
                ServiceError::NotFound(msg)
            })?;

        if order.status != OrderStatus::Pending {
            return Err(ServiceError::InvalidOperation(format!(
                "Order {} is not pending",
                self.order_id
            )));
        }

        let mut order: order_entity::ActiveModel = order.into();
        order.status = Set(OrderStatus::Processing);
        order.updated_at = Set(Utc::now());

        order.update(db).await.map_err(|e| {
            let msg = format!("Failed to update Order {}: {}", self.order_id, e);
            error!("{}", msg);
            ServiceError::db_error(e)
        })?;
    }

    async fn log_and_trigger_event(
        &self,
        event_sender: &EventSender,
        confirmed_order: &order_entity::Model,
    ) -> Result<(), ServiceError> {
        info!(order_id = %self.order_id, "Order confirmed successfully");

        event_sender
            .send(Event::OrderUpdated {
                order_id: self.order_id,
                checkout_session_id: None,
                status: None,
                refunds: vec![],
            })
            .await
            .map_err(|e| {
                ORDER_CONFIRM_FAILURES.inc();
                let msg = format!("Failed to send event for confirmed order: {}", e);
                error!("{}", msg);
                ServiceError::EventError(msg)
            })
    }
}
