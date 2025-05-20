use std::sync::Arc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;
use async_trait::async_trait;
use tracing::{info, instrument};

use crate::{
    db::DbPool,
    errors::ServiceError,
    events::{Event, EventSender},
    commands::Command,
};

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct QuarantineInventoryCommand {
    pub product_id: Uuid,
    #[validate(range(min = 1))]
    pub quantity: i32,
    pub reason: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct QuarantineInventoryResult {
    pub product_id: Uuid,
    pub quantity: i32,
}

#[async_trait]
impl Command for QuarantineInventoryCommand {
    type Result = QuarantineInventoryResult;

    #[instrument(skip(self, _db_pool, event_sender))]
    async fn execute(
        &self,
        _db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        self.validate()
            .map_err(|e| ServiceError::ValidationError(e.to_string()))?;

        info!(product_id = %self.product_id, quantity = self.quantity, "Quarantining inventory");

        event_sender
            .send(Event::with_data(format!("inventory_quarantined:{}:{}", self.product_id, self.quantity)))
            .await
            .map_err(ServiceError::EventError)?;

        Ok(QuarantineInventoryResult { product_id: self.product_id, quantity: self.quantity })
    }
}

