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
pub struct QualityCheckCommand {
    pub order_id: Uuid,
    #[validate(range(min = 0, max = 100))]
    pub score: i32,
    pub notes: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct QualityCheckResult {
    pub order_id: Uuid,
    pub passed: bool,
}

#[async_trait]
impl Command for QualityCheckCommand {
    type Result = QualityCheckResult;

    #[instrument(skip(self, _db_pool, event_sender))]
    async fn execute(
        &self,
        _db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        self.validate()
            .map_err(|e| ServiceError::ValidationError(e.to_string()))?;

        let passed = self.score >= 80;
        info!(order_id = %self.order_id, score = self.score, "Quality check completed");

        event_sender
            .send(Event::with_data(format!(
                "quality_check:{}:{}",
                self.order_id, passed
            )))
            .await
            .map_err(ServiceError::EventError)?;

        Ok(QualityCheckResult {
            order_id: self.order_id,
            passed,
        })
    }
}
