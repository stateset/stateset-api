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
pub struct MergeCustomersCommand {
    pub master_id: Uuid,
    pub duplicate_id: Uuid,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MergeCustomersResult {
    pub merged_into: Uuid,
}

#[async_trait]
impl Command for MergeCustomersCommand {
    type Result = MergeCustomersResult;

    #[instrument(skip(self, _db_pool, event_sender))]
    async fn execute(
        &self,
        _db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        info!(
            "Merged customer {} into {}",
            self.duplicate_id, self.master_id
        );
        event_sender
            .send(Event::with_data(format!(
                "customers_merged:{}:{}",
                self.master_id, self.duplicate_id
            )))
            .await
            .map_err(ServiceError::EventError)?;

        Ok(MergeCustomersResult {
            merged_into: self.master_id,
        })
    }
}
