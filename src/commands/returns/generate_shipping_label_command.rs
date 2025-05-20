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
pub struct GenerateShippingLabelCommand {
    pub return_id: Uuid,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GenerateShippingLabelResult {
    pub label_url: String,
}

#[async_trait]
impl Command for GenerateShippingLabelCommand {
    type Result = GenerateShippingLabelResult;

    #[instrument(skip(self, _db_pool, event_sender))]
    async fn execute(
        &self,
        _db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        self.validate()
            .map_err(|e| ServiceError::ValidationError(e.to_string()))?;

        info!(return_id = %self.return_id, "Generating shipping label for return");

        let label_url = format!("https://example.com/labels/{}.pdf", self.return_id);

        event_sender
            .send(Event::with_data(format!("return_label_generated:{}", self.return_id)))
            .await
            .map_err(ServiceError::EventError)?;

        Ok(GenerateShippingLabelResult { label_url })
    }
}

