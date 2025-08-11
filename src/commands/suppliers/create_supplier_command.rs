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
pub struct CreateSupplierCommand {
    #[validate(length(min = 1))]
    pub name: String,
    #[validate(length(min = 1))]
    pub contact_name: String,
    #[validate(email)]
    pub email: String,
    #[validate(length(min = 10))]
    pub phone: String,
    pub address: String,
    pub category: String,
    pub payment_terms: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateSupplierResult {
    pub id: Uuid,
}

#[async_trait]
impl Command for CreateSupplierCommand {
    type Result = CreateSupplierResult;

    #[instrument(skip(self, _db_pool, event_sender))]
    async fn execute(
        &self,
        _db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        self.validate()
            .map_err(|e| ServiceError::ValidationError(e.to_string()))?;

        let supplier_id = Uuid::new_v4();

        info!("Supplier created: {}", supplier_id);
        event_sender
            .send(Event::with_data(format!(
                "supplier_created:{}",
                supplier_id
            )))
            .await
            .map_err(ServiceError::EventError)?;

        Ok(CreateSupplierResult { id: supplier_id })
    }
}
