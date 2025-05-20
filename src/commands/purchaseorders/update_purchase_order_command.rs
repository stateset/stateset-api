use std::sync::Arc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;
use async_trait::async_trait;
use tracing::{info, instrument};
use chrono::NaiveDateTime;

use crate::{
    db::DbPool,
    errors::ServiceError,
    events::{Event, EventSender},
    commands::Command,
};

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct UpdatePurchaseOrderCommand {
    pub id: Uuid,
    pub expected_delivery_date: Option<NaiveDateTime>,
    pub shipping_address: Option<String>,
    pub notes: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdatePurchaseOrderResult {
    pub id: Uuid,
}

#[async_trait]
impl Command for UpdatePurchaseOrderCommand {
    type Result = UpdatePurchaseOrderResult;

    #[instrument(skip(self, _db_pool, event_sender))]
    async fn execute(
        &self,
        _db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        self.validate()
            .map_err(|e| ServiceError::ValidationError(e.to_string()))?;

        info!(purchase_order_id = %self.id, "Purchase order updated");
        event_sender
            .send(Event::PurchaseOrderUpdated(self.id))
            .await
            .map_err(ServiceError::EventError)?;

        Ok(UpdatePurchaseOrderResult { id: self.id })
    }
}
