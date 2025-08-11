use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{info, instrument};
use uuid::Uuid;
use validator::Validate;

use crate::{
    commands::Command,
    db::DbPool,
    errors::ServiceError,
    events::{Event, EventSender},
};
#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct TransferOrderCommand {
    pub order_id: Uuid,
    pub from_warehouse: Uuid,
    pub to_warehouse: Uuid,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct TransferOrderResult {
    pub order_id: Uuid,
    pub from_warehouse: Uuid,
    pub to_warehouse: Uuid,
}

#[async_trait]
impl Command for TransferOrderCommand {
    type Result = TransferOrderResult;
    #[instrument(skip(self, _db_pool, event_sender))]
    async fn execute(
        &self,
        _db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        info!(
            order_id = %self.order_id,
            from = %self.from_warehouse,
            to = %self.to_warehouse,
            "Transferring order"
        );
        event_sender
            .send(Event::with_data(format!(
                "order_transferred:{}:{}:{}",
                self.order_id, self.from_warehouse, self.to_warehouse
            )))
            .await
            .map_err(ServiceError::EventError)?;
        Ok(TransferOrderResult {
            order_id: self.order_id,
            from_warehouse: self.from_warehouse,
            to_warehouse: self.to_warehouse,
        })
    }
}
