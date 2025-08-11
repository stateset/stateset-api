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
pub struct ReleaseFromQuarantineCommand {
    pub product_id: Uuid,
    #[validate(range(min = 1))]
    pub quantity: i32,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct ReleaseFromQuarantineResult {
    pub product_id: Uuid,
    pub quantity: i32,
}

#[async_trait]
impl Command for ReleaseFromQuarantineCommand {
    type Result = ReleaseFromQuarantineResult;
    #[instrument(skip(self, _db_pool, event_sender))]
    async fn execute(
        &self,
        _db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        self.validate()
            .map_err(|e| ServiceError::ValidationError(e.to_string()))?;
        info!(product_id = %self.product_id, quantity = self.quantity, "Releasing inventory from quarantine");
        event_sender
            .send(Event::with_data(format!(
                "inventory_quarantine_released:{}:{}",
                self.product_id, self.quantity
            )))
            .await
            .map_err(ServiceError::EventError)?;
        Ok(ReleaseFromQuarantineResult {
            product_id: self.product_id,
            quantity: self.quantity,
        })
    }
}
