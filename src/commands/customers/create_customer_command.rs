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
pub struct CreateCustomerCommand {
    #[validate(length(min = 1))]
    pub name: String,
    #[validate(email)]
    pub email: String,
    pub phone: Option<String>,
    pub address: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateCustomerResult {
    pub id: Uuid,
}

#[async_trait]
impl Command for CreateCustomerCommand {
    type Result = CreateCustomerResult;

    #[instrument(skip(self, _db_pool, event_sender))]
    async fn execute(
        &self,
        _db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        self.validate()
            .map_err(|e| ServiceError::ValidationError(e.to_string()))?;

        let customer_id = Uuid::new_v4();
        info!("Customer created: {}", customer_id);
        event_sender
            .send(Event::with_data(format!(
                "customer_created:{}",
                customer_id
            )))
            .await
            .map_err(ServiceError::EventError)?;

        Ok(CreateCustomerResult { id: customer_id })
    }
}
