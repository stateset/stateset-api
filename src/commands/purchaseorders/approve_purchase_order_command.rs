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
pub struct ApprovePurchaseOrderCommand {
    pub id: Uuid,
    pub approver_id: Uuid,
    pub notes: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ApprovePurchaseOrderResult {
    pub id: Uuid,
}

#[async_trait]
impl Command for ApprovePurchaseOrderCommand {
    type Result = ApprovePurchaseOrderResult;

    #[instrument(skip(self, _db_pool, event_sender))]
    async fn execute(
        &self,
        _db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        self.validate()
            .map_err(|e| ServiceError::ValidationError(e.to_string()))?;

        info!(purchase_order_id = %self.id, approver = %self.approver_id, "Purchase order approved");
        event_sender
            .send(Event::PurchaseOrderApproved(self.id))
            .await
            .map_err(ServiceError::EventError)?;

        Ok(ApprovePurchaseOrderResult { id: self.id })
    }
}
