use crate::{
    commands::Command,
    events::{Event, EventSender},
};
use crate::{
    db::DbPool,
    errors::ServiceError,
    models::shipment::{self, ShipmentStatus},
};
use sea_orm::{entity::*, ActiveValue, EntityTrait};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info, instrument};
use uuid::Uuid;
use validator::Validate;

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct HoldShipmentCommand {
    pub shipment_id: Uuid,
    pub reason: String, // Reason for placing the shipment on hold
}

#[async_trait::async_trait]
impl Command for HoldShipmentCommand {
    type Result = shipment::Model;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(
        &self,
        db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        let db = db_pool.clone();

        let updated_shipment = self.hold_shipment(&db).await?;

        self.log_and_trigger_event(event_sender, &updated_shipment)
            .await?;

        Ok(updated_shipment)
    }
}

impl HoldShipmentCommand {
    async fn hold_shipment(
        &self,
        db: &sea_orm::DatabaseConnection,
    ) -> Result<shipment::Model, ServiceError> {
        let mut shipment: shipment::ActiveModel = shipment::Entity::find_by_id(self.shipment_id)
            .one(db)
            .await
            .map_err(|e| {
                error!(
                    "Failed to fetch shipment with ID {}: {}",
                    self.shipment_id, e
                );
                ServiceError::db_error(format!("Failed to fetch shipment: {}", e))
            })?
            .ok_or_else(|| {
                error!("Shipment with ID {} not found", self.shipment_id);
                ServiceError::NotFound(format!("Shipment with ID {} not found", self.shipment_id))
            })?
            .into();

        shipment.status = ActiveValue::Set(ShipmentStatus::OnHold); // Setting status to OnHold

        shipment.update(db).await.map_err(|e| {
            error!("Failed to hold shipment ID {}: {}", self.shipment_id, e);
            ServiceError::db_error(format!("Failed to hold shipment: {}", e))
        })
    }

    async fn log_and_trigger_event(
        &self,
        event_sender: Arc<EventSender>,
        _shipment: &shipment::Model,
    ) -> Result<(), ServiceError> {
        info!(
            "Shipment ID: {} placed on hold. Reason: {}",
            self.shipment_id, self.reason
        );
        event_sender
            .send(Event::ShipmentOnHold(self.shipment_id))
            .await
            .map_err(|e| {
                error!(
                    "Failed to send ShipmentOnHold event for shipment ID {}: {}",
                    self.shipment_id, e
                );
                ServiceError::EventError(e.to_string())
            })
    }
}
