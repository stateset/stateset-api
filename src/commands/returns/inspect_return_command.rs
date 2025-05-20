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
            .send(Event::ReturnProcessed(self.return_id))
            .await
            .map_err(ServiceError::EventError)?;

        Ok(InspectReturnResult { inspected: true })
    }
}

