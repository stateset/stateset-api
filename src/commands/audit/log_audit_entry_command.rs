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
pub struct LogAuditEntryCommand {
    pub user_id: Uuid,
    #[validate(length(min = 1))]
    pub action: String,
    pub entity_type: String,
    pub entity_id: Uuid,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct LogAuditEntryResult {
    pub entry_id: Uuid,
}

#[async_trait]
impl Command for LogAuditEntryCommand {
    type Result = LogAuditEntryResult;
    #[instrument(skip(self, _db_pool, event_sender))]
    async fn execute(
        &self,
        _db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        self.validate()
            .map_err(|e| ServiceError::ValidationError(e.to_string()))?;
        let entry_id = Uuid::new_v4();
        info!(entry_id = %entry_id, "Audit entry logged");
        event_sender
            .send(Event::with_data(format!(
                "audit:{}:{}:{}:{}",
                entry_id, self.user_id, self.action, self.entity_id
            )))
            .await
            .map_err(ServiceError::EventError)?;
        Ok(LogAuditEntryResult { entry_id })
    }
}
