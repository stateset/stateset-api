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
pub struct AddCustomerNoteCommand {
    pub customer_id: Uuid,
    #[validate(length(min = 1))]
    pub note: String,
    pub created_by: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AddCustomerNoteResult {
    pub customer_id: Uuid,
    pub note: String,
}

#[async_trait]
impl Command for AddCustomerNoteCommand {
    type Result = AddCustomerNoteResult;

    #[instrument(skip(self, _db_pool, event_sender))]
    async fn execute(
        &self,
        _db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        self.validate()
            .map_err(|e| ServiceError::ValidationError(e.to_string()))?;

        info!("Customer note added: {}", self.customer_id);
        event_sender
            .send(Event::with_data(format!(
                "customer_note_added:{}",
                self.customer_id
            )))
            .await
            .map_err(ServiceError::EventError)?;

        Ok(AddCustomerNoteResult {
            customer_id: self.customer_id,
            note: self.note.clone(),
        })
    }
}
