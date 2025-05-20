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
pub struct UnsuspendCustomerCommand {
    pub id: Uuid,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UnsuspendCustomerResult {
    pub id: Uuid,
}

#[async_trait]
impl Command for UnsuspendCustomerCommand {
    type Result = UnsuspendCustomerResult;

    #[instrument(skip(self, _db_pool, event_sender))]
    async fn execute(
        &self,
        _db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        info!("Customer unsuspended: {}", self.id);
        event_sender
            .send(Event::with_data(format!("customer_unsuspended:{}", self.id)))
            .await
            .map_err(ServiceError::EventError)?;

        Ok(UnsuspendCustomerResult { id: self.id })
    }
}
