use crate::{
    commands::Command,
    db::DbPool,
    errors::ServiceError,
    events::{Event, EventSender},
    models::{shipment, Shipment},
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sea_orm::{entity::*, query::*, ActiveValue, ColumnTrait, EntityTrait};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info, instrument};
use uuid::Uuid;
use validator::Validate;

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct UpdateShipmentCommand {
    pub id: Uuid,
    pub recipient_name: Option<String>,
    pub shipping_address: Option<String>,
    pub carrier: Option<String>,
    pub tracking_number: Option<String>,
    pub status: Option<String>,
    pub estimated_delivery_date: Option<DateTime<Utc>>,
}

#[async_trait::async_trait]
impl Command for UpdateShipmentCommand {
    type Result = Shipment;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(
        &self,
        db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        let db = db_pool.as_ref();

        let updated_shipment = self.update_shipment(&db).await?;

        self.log_and_trigger_event(event_sender, &updated_shipment)
            .await?;

        Ok(updated_shipment)
    }
}

impl UpdateShipmentCommand {
    async fn update_shipment(
        &self,
        db: &sea_orm::DatabaseConnection,
    ) -> Result<Shipment, ServiceError> {
        let mut shipment: shipment::ActiveModel = shipment::Entity::find_by_id(self.id)
            .one(db)
            .await
            .map_err(|e| {
                error!("Failed to fetch shipment with ID {}: {}", self.id, e);
                ServiceError::DatabaseError(format!("Failed to fetch shipment: {}", e))
            })?
            .ok_or_else(|| {
                error!("Shipment with ID {} not found", self.id);
                ServiceError::NotFound
            })?
            .into();

        // Update only fields that are provided
        if let Some(recipient_name) = &self.recipient_name {
            shipment.recipient_name = ActiveValue::Set(recipient_name.clone());
        }
        if let Some(shipping_address) = &self.shipping_address {
            shipment.shipping_address = ActiveValue::Set(shipping_address.clone());
        }
        if let Some(carrier) = &self.carrier {
            shipment.carrier = ActiveValue::Set(Some(carrier.clone()));
        }
        if let Some(tracking_number) = &self.tracking_number {
            shipment.tracking_number = ActiveValue::Set(Some(tracking_number.clone()));
        }
        if let Some(status) = &self.status {
            shipment.status = ActiveValue::Set(status.clone());
        }
        if let Some(estimated_delivery_date) = &self.estimated_delivery_date {
            shipment.estimated_delivery_date = ActiveValue::Set(Some(estimated_delivery_date.naive_utc()));
        }

        shipment.updated_at = ActiveValue::Set(Utc::now());

        shipment.update(db).await.map_err(|e| {
            error!("Failed to update shipment ID {}: {}", self.id, e);
            ServiceError::DatabaseError(format!("Failed to update shipment: {}", e))
        })
    }

    async fn log_and_trigger_event(
        &self,
        event_sender: Arc<EventSender>,
        updated_shipment: &Shipment,
    ) -> Result<(), ServiceError> {
        info!("Shipment updated: {}", self.id);
        
        event_sender
            .send(Event::ShipmentUpdated(self.id))
            .await
            .map_err(|e| {
                error!("Failed to send ShipmentUpdated event for shipment ID {}: {}", self.id, e);
                ServiceError::EventError(e.to_string())
            })
    }
}