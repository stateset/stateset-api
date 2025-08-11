use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{info, instrument};
use uuid::Uuid;

use crate::{
    commands::Command,
    db::DbPool,
    errors::ServiceError,
    events::{Event, EventSender},
};

#[derive(Debug, Serialize, Deserialize)]
pub struct ArchiveCustomerCommand {
    pub id: Uuid,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ArchiveCustomerResult {
    pub id: Uuid,
}

#[async_trait]
impl Command for ArchiveCustomerCommand {
    type Result = ArchiveCustomerResult;

    #[instrument(skip(self, _db_pool, event_sender))]
    async fn execute(
        &self,
        _db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        info!("Customer archived: {}", self.id);
        event_sender
            .send(Event::with_data(format!("customer_archived:{}", self.id)))
            .await
            .map_err(ServiceError::EventError)?;

        Ok(ArchiveCustomerResult { id: self.id })
    }
}
