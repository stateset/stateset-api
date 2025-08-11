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
pub struct KitComponent {
    pub product_id: Uuid,
    #[validate(range(min = 1))]
    pub quantity: i32,
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct CreateKitCommand {
    pub kit_id: Uuid,
    #[validate(length(min = 1))]
    pub components: Vec<KitComponent>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateKitResult {
    pub kit_id: Uuid,
    pub components: usize,
}

#[async_trait]
impl Command for CreateKitCommand {
    type Result = CreateKitResult;

    #[instrument(skip(self, _db_pool, event_sender))]
    async fn execute(
        &self,
        _db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        self.validate()
            .map_err(|e| ServiceError::ValidationError(e.to_string()))?;

        info!(kit_id = %self.kit_id, "Creating kit");

        event_sender
            .send(Event::with_data(format!("kit_created:{}", self.kit_id)))
            .await
            .map_err(ServiceError::EventError)?;

        Ok(CreateKitResult {
            kit_id: self.kit_id,
            components: self.components.len(),
        })
    }
}
