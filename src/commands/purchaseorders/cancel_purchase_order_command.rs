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
pub struct CancelPurchaseOrderCommand {
    pub id: Uuid,
    #[validate(length(min = 1))]
    pub reason: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CancelPurchaseOrderResult {
    pub id: Uuid,
}

#[async_trait]
impl Command for CancelPurchaseOrderCommand {
    type Result = CancelPurchaseOrderResult;

    #[instrument(skip(self, _db_pool, event_sender))]
    async fn execute(
        &self,
        _db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        self.validate()
            .map_err(|e| ServiceError::ValidationError(e.to_string()))?;

        info!(purchase_order_id = %self.id, "Purchase order cancelled");
        event_sender
            .send(Event::PurchaseOrderCancelled(self.id))
            .await
            .map_err(ServiceError::EventError)?;

        Ok(CancelPurchaseOrderResult { id: self.id })
    }
}
