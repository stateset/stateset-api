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
pub struct ReceivePurchaseOrderCommand {
    pub id: Uuid,
    pub received_by: Uuid,
    pub notes: Option<String>,
    pub items_received: Vec<(Uuid, i32, Option<String>)>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ReceivePurchaseOrderResult {
    pub id: Uuid,
}

#[async_trait]
impl Command for ReceivePurchaseOrderCommand {
    type Result = ReceivePurchaseOrderResult;

    #[instrument(skip(self, _db_pool, event_sender))]
    async fn execute(
        &self,
        _db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        self.validate()
            .map_err(|e| ServiceError::ValidationError(e.to_string()))?;

        info!(purchase_order_id = %self.id, "Purchase order received");
        // TODO: Add PurchaseOrderReceived event when available
        // event_sender
        //     .send(Event::PurchaseOrderReceived(self.id))
        //     .await
        //     .map_err(ServiceError::EventError)?;

        Ok(ReceivePurchaseOrderResult { id: self.id })
    }
}
