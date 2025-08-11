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
pub struct UpdateCustomerCommand {
    pub id: Uuid,
    #[validate(length(min = 1))]
    pub name: Option<String>,
    #[validate(email)]
    pub email: Option<String>,
    pub phone: Option<String>,
    pub address: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateCustomerResult {
    pub id: Uuid,
}

#[async_trait]
impl Command for UpdateCustomerCommand {
    type Result = UpdateCustomerResult;

    #[instrument(skip(self, _db_pool, event_sender))]
    async fn execute(
        &self,
        _db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        self.validate()
            .map_err(|e| ServiceError::ValidationError(e.to_string()))?;

        info!("Customer updated: {}", self.id);
        event_sender
            .send(Event::with_data(format!("customer_updated:{}", self.id)))
            .await
            .map_err(ServiceError::EventError)?;

        Ok(UpdateCustomerResult { id: self.id })
    }
}
