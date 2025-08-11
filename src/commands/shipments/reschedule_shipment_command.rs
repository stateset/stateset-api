use uuid::Uuid;
use crate::{
    commands::Command,
    events::{Event, EventSender},
};
use crate::{
    db::DbPool,
    errors::ServiceError,
    models::{shipment, Shipment},
};
use async_trait::async_trait;
use chrono::NaiveDateTime;
use sea_orm::{entity::*, query::*, ActiveValue, ColumnTrait, EntityTrait};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info, instrument};
use validator::Validate;

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct RescheduleShipmentCommand {
    pub shipment_id: Uuid,

    pub new_scheduled_date: NaiveDateTime, // The new date and time for the shipment
}

#[async_trait::async_trait]
impl Command for RescheduleShipmentCommand {
    type Result = Shipment;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(
        &self,
        db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        let db = db_pool.clone();

        let updated_shipment = self.reschedule_shipment(&db).await?;

        self.log_and_trigger_event(event_sender, &updated_shipment)
            .await?;

        Ok(updated_shipment)
    }
}

impl RescheduleShipmentCommand {
    async fn reschedule_shipment(
        &self,
        db: &sea_orm::DatabaseConnection,
    ) -> Result<Shipment, ServiceError> {
        let mut shipment: shipment::ActiveModel = shipment::Entity::find_by_id(self.shipment_id)
            .one(db)
            .await
            .map_err(|e| {
                error!(
                    "Failed to fetch shipment with ID {}: {}",
                    self.shipment_id, e
                );
                ServiceError::DatabaseError(format!("Failed to fetch shipment: {}", e))
            })?
            .ok_or_else(|| {
                error!("Shipment with ID {} not found", self.shipment_id);
                ServiceError::NotFound
            })?
            .into();

        shipment.scheduled_date = ActiveValue::Set(self.new_scheduled_date);

        shipment.update(db).await.map_err(|e| {
            error!(
                "Failed to reschedule shipment ID {}: {}",
                self.shipment_id, e
            );
            ServiceError::DatabaseError(format!("Failed to reschedule shipment: {}", e))
        })
    }

    async fn log_and_trigger_event(
        &self,
        event_sender: Arc<EventSender>,
        shipment: &Shipment,
    ) -> Result<(), ServiceError> {
        info!(
            "Shipment ID: {} rescheduled to: {}",
            self.shipment_id, self.new_scheduled_date
        );
        event_sender
            .send(Event::ShipmentRescheduled(
                self.shipment_id,
                self.new_scheduled_date,
            ))
            .await
            .map_err(|e| {
                error!(
                    "Failed to send ShipmentRescheduled event for shipment ID {}: {}",
                    self.shipment_id, e
                );
                ServiceError::EventError(e.to_string())
            })
    }
}
