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
pub struct PickItem {
    pub product_id: Uuid,
    #[validate(range(min = 1))]
    pub quantity: i32,
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct PickItemsCommand {
    pub order_id: Uuid,
    pub picker_id: Option<Uuid>,
    #[validate(length(min = 1))]
    pub items: Vec<PickItem>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PickItemsResult {
    pub items_picked: usize,
}

#[async_trait]
impl Command for PickItemsCommand {
    type Result = PickItemsResult;
    #[instrument(skip(self, _db_pool, event_sender))]
    async fn execute(
        &self,
        _db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        self.validate()
            .map_err(|e| ServiceError::ValidationError(e.to_string()))?;
        info!(order_id = %self.order_id, "Picking items");
        event_sender
            .send(Event::with_data(format!("picked_items:{}", self.order_id)))
            .await
            .map_err(|e| ServiceError::EventError(e))?;
        Ok(PickItemsResult {
            items_picked: self.items.len(),
        })
    }
}
