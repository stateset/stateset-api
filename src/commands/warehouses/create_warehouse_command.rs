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
pub struct CreateWarehouseCommand {
    #[validate(length(min = 1))]
    pub name: String,
    pub location: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateWarehouseResult {
    pub id: Uuid,
}

#[async_trait]
impl Command for CreateWarehouseCommand {
    type Result = CreateWarehouseResult;

    #[instrument(skip(self, _db_pool, event_sender))]
    async fn execute(
        &self,
        _db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        self.validate()
            .map_err(|e| ServiceError::ValidationError(e.to_string()))?;

        let warehouse_id = Uuid::new_v4();
        info!("Warehouse created: {}", warehouse_id);
        event_sender
            .send(Event::with_data(format!(
                "warehouse_created:{}",
                warehouse_id
            )))
            .await
            .map_err(ServiceError::EventError)?;

        Ok(CreateWarehouseResult { id: warehouse_id })
    }
}
