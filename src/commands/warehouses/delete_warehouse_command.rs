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
pub struct DeleteWarehouseCommand {
    pub id: Uuid,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeleteWarehouseResult {
    pub id: Uuid,
}

#[async_trait]
impl Command for DeleteWarehouseCommand {
    type Result = DeleteWarehouseResult;

    #[instrument(skip(self, _db_pool, event_sender))]
    async fn execute(
        &self,
        _db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        info!("Warehouse deleted: {}", self.id);
        event_sender
            .send(Event::with_data(format!("warehouse_deleted:{}", self.id)))
            .await
            .map_err(ServiceError::EventError)?;

        Ok(DeleteWarehouseResult { id: self.id })
    }
}
