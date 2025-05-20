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
pub struct SuspendCustomerCommand {
    pub id: Uuid,
    pub reason: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SuspendCustomerResult {
    pub id: Uuid,
}

#[async_trait]
impl Command for SuspendCustomerCommand {
    type Result = SuspendCustomerResult;

    #[instrument(skip(self, _db_pool, event_sender))]
    async fn execute(
        &self,
        _db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        info!("Customer suspended: {}", self.id);
        event_sender
            .send(Event::with_data(format!("customer_suspended:{}", self.id)))
            .await
            .map_err(ServiceError::EventError)?;

        Ok(SuspendCustomerResult { id: self.id })
    }
}
