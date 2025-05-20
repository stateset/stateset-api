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
pub struct MarkLotExpiredCommand {
    pub lot_id: Uuid,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MarkLotExpiredResult {
    pub lot_id: Uuid,
}

#[async_trait]
impl Command for MarkLotExpiredCommand {
    type Result = MarkLotExpiredResult;

    #[instrument(skip(self, _db_pool, event_sender))]
    async fn execute(
        &self,
        _db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        self.validate()
            .map_err(|e| ServiceError::ValidationError(e.to_string()))?;

        info!(lot_id = %self.lot_id, "Marking lot expired");

        event_sender
            .send(Event::with_data(format!("lot_expired:{}", self.lot_id)))
            .await
            .map_err(ServiceError::EventError)?;

        Ok(MarkLotExpiredResult { lot_id: self.lot_id })
    }
}

