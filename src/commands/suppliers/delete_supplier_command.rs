use std::sync::Arc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use async_trait::async_trait;
use tracing::{info, instrument};

use crate::{
    db::DbPool,
    errors::ServiceError,
    events::{Event, EventSender},
    commands::Command,
};

#[derive(Debug, Serialize, Deserialize)]
pub struct DeleteSupplierCommand {
    pub id: Uuid,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeleteSupplierResult {
    pub id: Uuid,
}

#[async_trait]
impl Command for DeleteSupplierCommand {
    type Result = DeleteSupplierResult;

    #[instrument(skip(self, _db_pool, event_sender))]
    async fn execute(
        &self,
        _db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        info!("Supplier deleted: {}", self.id);
        event_sender
            .send(Event::with_data(format!("supplier_deleted:{}", self.id)))
            .await
            .map_err(ServiceError::EventError)?;

        Ok(DeleteSupplierResult { id: self.id })
    }
}
