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
pub struct CreateCarrierCommand {
    #[validate(length(min = 1))]
    pub name: String,
    pub tracking_url: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateCarrierResult {
    pub id: Uuid,
}

#[async_trait]
impl Command for CreateCarrierCommand {
    type Result = CreateCarrierResult;

    #[instrument(skip(self, _db_pool, event_sender))]
    async fn execute(
        &self,
        _db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        self.validate()
            .map_err(|e| ServiceError::ValidationError(e.to_string()))?;

        let carrier_id = Uuid::new_v4();
        info!("Carrier created: {}", carrier_id);
        event_sender
            .send(Event::with_data(format!("carrier_created:{}", carrier_id)))
            .await
            .map_err(ServiceError::EventError)?;

        Ok(CreateCarrierResult { id: carrier_id })
    }
}
