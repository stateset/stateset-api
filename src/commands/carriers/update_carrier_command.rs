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
pub struct UpdateCarrierCommand {
    pub id: Uuid,
    pub name: Option<String>,
    pub tracking_url: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateCarrierResult {
    pub id: Uuid,
}

#[async_trait]
impl Command for UpdateCarrierCommand {
    type Result = UpdateCarrierResult;

    #[instrument(skip(self, _db_pool, event_sender))]
    async fn execute(
        &self,
        _db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        self.validate()
            .map_err(|e| ServiceError::ValidationError(e.to_string()))?;

        info!("Carrier updated: {}", self.id);
        event_sender
            .send(Event::with_data(format!("carrier_updated:{}", self.id)))
            .await
            .map_err(ServiceError::EventError)?;

        Ok(UpdateCarrierResult { id: self.id })
    }
}
