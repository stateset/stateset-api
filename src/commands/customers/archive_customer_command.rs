use std::sync::Arc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use async_trait::async_trait;
use tracing::{info, instrument};

use crate::{
    db::DbPool,
    errors::ServiceError,
    events::{Event, EventSender},
    commands::Command,
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
