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
pub struct InspectReturnCommand {
    pub return_id: Uuid,
    pub notes: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct InspectReturnResult {
    pub inspected: bool,
}

#[async_trait]
impl Command for InspectReturnCommand {
    type Result = InspectReturnResult;

    #[instrument(skip(self, _db_pool, event_sender))]
    async fn execute(
        &self,
        _db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        self.validate()
            .map_err(|e| ServiceError::ValidationError(e.to_string()))?;

        info!(return_id = %self.return_id, "Inspecting return");

        event_sender
            .send(Event::ReturnUpdated(self.return_id))
            .await
            .map_err(|e| ServiceError::EventError(e.to_string()))?;

        Ok(InspectReturnResult { inspected: true })
    }
}
