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
pub struct UpdateLotCommand {
    pub lot_id: Uuid,
    #[validate(range(min = 0))]
    pub quantity: i32,
    pub expiration_date: Option<chrono::NaiveDate>,
    pub notes: Option<String>,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateLotResult {
    pub lot_id: Uuid,
}

#[async_trait]
impl Command for UpdateLotCommand {
    type Result = UpdateLotResult;
    #[instrument(skip(self, _db_pool, event_sender))]
    async fn execute(
        &self,
        _db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        self.validate()
            .map_err(|e| ServiceError::ValidationError(e.to_string()))?;
        info!(lot_id = %self.lot_id, "Updating lot");
        event_sender
            .send(Event::with_data(format!("lot_updated:{}", self.lot_id)))
            .await
            .map_err(ServiceError::EventError)?;
        Ok(UpdateLotResult {
            lot_id: self.lot_id,
        })
    }
}
