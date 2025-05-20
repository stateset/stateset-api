use std::sync::Arc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use async_trait::async_trait;
use tracing::{info, instrument};
use validator::Validate;

use crate::{
    db::DbPool,
    errors::ServiceError,
    events::{Event, EventSender},
    commands::Command,
};

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct FlagCustomerCommand {
    pub id: Uuid,
    #[validate(length(min = 1))]
    pub flag: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FlagCustomerResult {
    pub id: Uuid,
    pub flag: String,
}

#[async_trait]
impl Command for FlagCustomerCommand {
    type Result = FlagCustomerResult;

    #[instrument(skip(self, _db_pool, event_sender))]
    async fn execute(
        &self,
        _db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        self.validate()
            .map_err(|e| ServiceError::ValidationError(e.to_string()))?;

        info!("Customer flagged: {} flag:{}", self.id, self.flag);
        event_sender
            .send(Event::with_data(format!("customer_flagged:{}:{}", self.id, self.flag)))
            .await
            .map_err(ServiceError::EventError)?;

        Ok(FlagCustomerResult { id: self.id, flag: self.flag.clone() })
    }
}

