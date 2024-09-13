use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use crate::{errors::ServiceError, db::DbPool, models::{shipment, Shipment, ShipmentStatus}};
use crate::events::{Event, EventSender};
use tracing::{info, error, instrument};
use sea_orm::{entity::*, query::*, ColumnTrait, EntityTrait, ActiveValue};
use chrono::Utc;

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct ConfirmShipmentDeliveryCommand {
    pub shipment_id: i32,
}

#[async_trait::async_trait]
impl Command for ConfirmShipmentDeliveryCommand {
    type Result = Shipment;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        let db = db_pool.clone();

        let updated_shipment = self.confirm_delivery(&db).await?;

        self.log_and_trigger_event(event_sender, &updated_shipment).await?;

        Ok(updated_shipment)
    }
}

impl ConfirmShipmentDeliveryCommand {
    async fn confirm_delivery(&self, db: &sea_orm::DatabaseConnection) -> Result<Shipment, ServiceError> {
        let mut shipment: shipment::ActiveModel = shipment::Entity::find_by_id(self.shipment_id)
            .one(db)
            .await
            .map_err(|e| {
                error!("Failed to fetch shipment with ID {}: {}", self.shipment_id, e);
                ServiceError::DatabaseError(format!("Failed to fetch shipment: {}", e))
            })?
            .ok_or_else(|| {
                error!("Shipment with ID {} not found", self.shipment_id);
                ServiceError::NotFound
            })?
            .into();

        shipment.status = ActiveValue::Set(ShipmentStatus::Delivered);
        shipment.delivered_at = ActiveValue::Set(Some(Utc::now()));

        shipment.update(db)
            .await
            .map_err(|e| {
                error!("Failed to confirm delivery for shipment ID {}: {}", self.shipment_id, e);
                ServiceError::DatabaseError(format!("Failed to confirm delivery: {}", e))
            })
    }

    async fn log_and_trigger_event(&self, event_sender: Arc<EventSender>, shipment: &Shipment) -> Result<(), ServiceError> {
        info!("Shipment ID: {} confirmed as delivered.", self.shipment_id);
        event_sender.send(Event::ShipmentDelivered(self.shipment_id))
            .await
            .map_err(|e| {
                error!("Failed to send ShipmentDelivered event for shipment ID {}: {}", self.shipment_id, e);
                ServiceError::EventError(e.to_string())
            })
    }
}
