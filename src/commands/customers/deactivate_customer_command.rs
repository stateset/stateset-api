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
pub struct DeactivateCustomerCommand {
    pub id: Uuid,
    pub reason: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeactivateCustomerResult {
    pub id: Uuid,
}

#[async_trait]
impl Command for DeactivateCustomerCommand {
    type Result = DeactivateCustomerResult;

    #[instrument(skip(self, _db_pool, event_sender))]
    async fn execute(
        &self,
        _db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        info!("Customer deactivated: {}", self.id);
        event_sender
            .send(Event::with_data(format!(
                "customer_deactivated:{}:{}",
                self.id,
                self.reason.clone().unwrap_or_default()
            )))
            .await
            .map_err(ServiceError::EventError)?;

        Ok(DeactivateCustomerResult { id: self.id })
    }
}
