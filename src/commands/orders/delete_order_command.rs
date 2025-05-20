use std::sync::Arc;
use sea_orm::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use async_trait::async_trait;
use tracing::{error, info, instrument};

use crate::{
    db::DbPool,
    errors::ServiceError,
    events::{Event, EventSender},
    models::{order_entity::{self, Entity as Order}},
    commands::Command,
};

#[derive(Debug, Serialize, Deserialize)]
pub struct DeleteOrderCommand {
    pub order_id: Uuid,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeleteOrderResult {
    pub id: Uuid,
    pub deleted: bool,
}

#[async_trait]
impl Command for DeleteOrderCommand {
    type Result = DeleteOrderResult;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(
        &self,
        db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        let db = db_pool.as_ref();

        let deleted = self.delete_order(db).await?;

        self.log_and_trigger_event(&event_sender).await?;

        Ok(DeleteOrderResult { id: self.order_id, deleted })
    }
}

impl DeleteOrderCommand {
    async fn delete_order(&self, db: &DatabaseConnection) -> Result<bool, ServiceError> {
        let res = Order::delete_by_id(self.order_id)
            .exec(db)
            .await
            .map_err(|e| {
                let msg = format!("Failed to delete order: {}", e);
                error!("{}", msg);
                ServiceError::DatabaseError(msg)
            })?;
        Ok(res.rows_affected > 0)
    }

    async fn log_and_trigger_event(
        &self,
        event_sender: &EventSender,
    ) -> Result<(), ServiceError> {
        info!(order_id = %self.order_id, "Order deleted successfully");
        event_sender
            .send(Event::OrderDeleted(self.order_id))
            .await
            .map_err(|e| {
                let msg = format!("Failed to send event for deleted order: {}", e);
                error!("{}", msg);
                ServiceError::EventError(msg)
            })
    }
}

