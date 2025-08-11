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
pub struct ReceiveShipmentItem {
    pub product_id: Uuid,
    #[validate(range(min = 1))]
    pub quantity: i32,
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct ReceiveShipmentCommand {
    pub shipment_id: Uuid,
    #[validate(length(min = 1))]
    pub items: Vec<ReceiveShipmentItem>,
    pub receiver_id: Option<Uuid>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ReceiveShipmentResult {
    pub shipment_id: Uuid,
    pub items_received: usize,
}

#[async_trait]
impl Command for ReceiveShipmentCommand {
    type Result = ReceiveShipmentResult;

    #[instrument(skip(self, _db_pool, event_sender))]
    async fn execute(
        &self,
        _db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        self.validate()
            .map_err(|e| ServiceError::ValidationError(e.to_string()))?;

        info!(shipment_id = %self.shipment_id, "Receiving shipment");

        event_sender
            .send(Event::with_data(format!(
                "shipment_received:{}",
                self.shipment_id
            )))
            .await
            .map_err(|e| ServiceError::EventError(e))?;

        Ok(ReceiveShipmentResult {
            shipment_id: self.shipment_id,
            items_received: self.items.len(),
        })
    }
}
