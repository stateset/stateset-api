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
pub struct PackageItem {
    pub product_id: Uuid,
    #[validate(range(min = 1))]
    pub quantity: i32,
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct PackageOrderCommand {
    pub order_id: Uuid,
    #[validate(length(min = 1))]
    pub items: Vec<PackageItem>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PackageOrderResult {
    pub order_id: Uuid,
    pub packages_created: usize,
}

#[async_trait]
impl Command for PackageOrderCommand {
    type Result = PackageOrderResult;

    #[instrument(skip(self, _db_pool, event_sender))]
    async fn execute(
        &self,
        _db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        self.validate()
            .map_err(|e| ServiceError::ValidationError(e.to_string()))?;

        info!(order_id = %self.order_id, "Packaging order");

        event_sender
            .send(Event::with_data(format!(
                "order_packaged:{}",
                self.order_id
            )))
            .await
            .map_err(ServiceError::EventError)?;

        Ok(PackageOrderResult {
            order_id: self.order_id,
            packages_created: self.items.len(),
        })
    }
}
