use async_trait::async_trait;
use chrono::{DateTime, Utc};
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
pub struct ScheduleMaintenanceCommand {
    pub asset_id: Uuid,
    pub scheduled_for: DateTime<Utc>,
    pub notes: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ScheduleMaintenanceResult {
    pub asset_id: Uuid,
    pub scheduled_for: DateTime<Utc>,
}

#[async_trait]
impl Command for ScheduleMaintenanceCommand {
    type Result = ScheduleMaintenanceResult;

    #[instrument(skip(self, _db_pool, event_sender))]
    async fn execute(
        &self,
        _db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        self.validate()
            .map_err(|e| ServiceError::ValidationError(e.to_string()))?;

        info!(asset_id = %self.asset_id, "Scheduling maintenance");

        event_sender
            .send(Event::with_data(format!(
                "maintenance_scheduled:{}:{}",
                self.asset_id, self.scheduled_for
            )))
            .await
            .map_err(ServiceError::EventError)?;

        Ok(ScheduleMaintenanceResult {
            asset_id: self.asset_id,
            scheduled_for: self.scheduled_for,
        })
    }
}
