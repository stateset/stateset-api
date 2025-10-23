use crate::{
    commands::Command,
    events::{Event, EventSender},
};
use crate::{db::DbPool, errors::ServiceError, models::shipment};
use chrono::{DateTime, NaiveDateTime, Utc};
use sea_orm::{entity::*, query::*, ActiveValue, EntityTrait};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info, instrument};
use uuid::Uuid;
use validator::Validate;

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct RescheduleShipmentCommand {
    pub shipment_id: Uuid,

    pub new_scheduled_date: NaiveDateTime, // The new date and time for the shipment
}

#[async_trait::async_trait]
impl Command for RescheduleShipmentCommand {
    type Result = shipment::Model;

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

        let rescheduled_at = DateTime::<Utc>::from_utc(self.new_scheduled_date, Utc);
        shipment.estimated_delivery = ActiveValue::Set(Some(rescheduled_at));
        shipment.updated_at = ActiveValue::Set(Utc::now());

        shipment.update(db).await.map_err(|e| {
            error!(
                "Failed to reschedule shipment ID {}: {}",
                self.shipment_id, e
            );
            ServiceError::db_error(format!("Failed to reschedule shipment: {}", e))
        })
    }

    async fn log_and_trigger_event(
        &self,
        event_sender: Arc<EventSender>,
        shipment: &shipment::Model,
    ) -> Result<(), ServiceError> {
        info!(
            "Shipment ID: {} rescheduled to: {}",
            self.shipment_id, self.new_scheduled_date
        );
        let rescheduled_at = DateTime::<Utc>::from_utc(self.new_scheduled_date, Utc);
        event_sender
            .send(Event::ShipmentRescheduled(self.shipment_id, rescheduled_at))
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
