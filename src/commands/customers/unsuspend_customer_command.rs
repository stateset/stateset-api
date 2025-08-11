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
            .send(Event::with_data(format!(
                "customer_unsuspended:{}",
                self.id
            )))
            .await
            .map_err(ServiceError::EventError)?;

        Ok(UnsuspendCustomerResult { id: self.id })
    }
}
