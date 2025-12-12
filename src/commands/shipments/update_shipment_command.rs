use crate::{
    commands::Command,
    db::DbPool,
    errors::ServiceError,
    events::{Event, EventSender},
    models::shipment::{self, ShipmentStatus},
};
use sea_orm::{entity::*, ActiveValue, EntityTrait};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info, instrument};
use uuid::Uuid;
use validator::Validate;

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct UpdateShipmentStatusCommand {
    pub shipment_id: Uuid,

    pub new_status: ShipmentStatus,
}

#[async_trait::async_trait]
impl Command for UpdateShipmentStatusCommand {
    type Result = shipment::Model;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(
        &self,
        db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        let db = db_pool.clone();

        let updated_shipment = self.update_shipment_status(&db).await?;

        self.log_and_trigger_event(event_sender, &updated_shipment)
            .await?;

        Ok(updated_shipment)
    }
}

impl UpdateShipmentStatusCommand {
    async fn update_shipment_status(
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

        shipment.status = ActiveValue::Set(self.new_status.clone());

        shipment.update(db).await.map_err(|e| {
            error!(
                "Failed to update shipment status for shipment ID {}: {}",
                self.shipment_id, e
            );
            ServiceError::db_error(format!("Failed to update shipment status: {}", e))
        })
    }

    async fn log_and_trigger_event(
        &self,
        event_sender: Arc<EventSender>,
        _updated_shipment: &shipment::Model,
    ) -> Result<(), ServiceError> {
        info!(
            "Shipment status updated for shipment ID: {}",
            self.shipment_id
        );
        event_sender
            .send(Event::ShipmentUpdated(self.shipment_id))
            .await
            .map_err(|e| {
                error!(
                    "Failed to send ShipmentUpdated event for shipment ID {}: {}",
                    self.shipment_id, e
                );
                ServiceError::EventError(e.to_string())
            })
    }
}
