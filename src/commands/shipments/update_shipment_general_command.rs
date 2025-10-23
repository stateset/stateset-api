use crate::{
    commands::Command,
    db::DbPool,
    errors::ServiceError,
    events::{Event, EventSender},
    models::shipment::{self, ShipmentStatus, ShippingCarrier},
};
use chrono::{DateTime, Utc};
use sea_orm::{entity::*, query::*, ActiveValue, EntityTrait};
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
    type Result = shipment::Model;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(
        &self,
        db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        let db = db_pool.as_ref();

        let updated_shipment = self.update_shipment(&db).await?;
        // Outbox enqueue (post-update)
        let payload = serde_json::json!({"shipment_id": updated_shipment.id.to_string()});
        let _ = crate::events::outbox::enqueue(
            db,
            "shipment",
            Some(updated_shipment.id),
            "ShipmentUpdated",
            &payload,
        )
        .await;

        self.log_and_trigger_event(event_sender, &updated_shipment)
            .await?;

        Ok(updated_shipment)
    }
}

impl UpdateShipmentCommand {
    async fn update_shipment(
        &self,
        db: &sea_orm::DatabaseConnection,
    ) -> Result<shipment::Model, ServiceError> {
        let mut shipment: shipment::ActiveModel = shipment::Entity::find_by_id(self.id)
            .one(db)
            .await
            .map_err(|e| {
                error!("Failed to fetch shipment with ID {}: {}", self.id, e);
                ServiceError::db_error(format!("Failed to fetch shipment: {}", e))
            })?
            .ok_or_else(|| {
                error!("Shipment with ID {} not found", self.id);
                ServiceError::NotFound(format!("Shipment with ID {} not found", self.id))
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
            let mapped_carrier = match carrier.to_lowercase().as_str() {
                "ups" => ShippingCarrier::UPS,
                "fedex" => ShippingCarrier::FedEx,
                "usps" => ShippingCarrier::USPS,
                "dhl" => ShippingCarrier::DHL,
                _ => ShippingCarrier::Other,
            };
            shipment.carrier = ActiveValue::Set(mapped_carrier);
        }
        if let Some(tracking_number) = &self.tracking_number {
            shipment.tracking_number = ActiveValue::Set(tracking_number.clone());
        }
        if let Some(status) = &self.status {
            let mapped_status = match status.to_lowercase().as_str() {
                "pending" => ShipmentStatus::Pending,
                "processing" => ShipmentStatus::Processing,
                "readytoship" | "ready_to_ship" | "ready-to-ship" => ShipmentStatus::ReadyToShip,
                "shipped" => ShipmentStatus::Shipped,
                "intransit" | "in_transit" | "in-transit" => ShipmentStatus::InTransit,
                "outfordelivery" | "out_for_delivery" | "out-for-delivery" => {
                    ShipmentStatus::OutForDelivery
                }
                "delivered" => ShipmentStatus::Delivered,
                "failed" => ShipmentStatus::Failed,
                "returned" => ShipmentStatus::Returned,
                "cancelled" | "canceled" => ShipmentStatus::Cancelled,
                "onhold" | "on_hold" | "on-hold" => ShipmentStatus::OnHold,
                other => {
                    return Err(ServiceError::InvalidInput(format!(
                        "Unknown shipment status: {}",
                        other
                    )));
                }
            };
            shipment.status = ActiveValue::Set(mapped_status);
        }
        if let Some(estimated_delivery_date) = &self.estimated_delivery_date {
            shipment.estimated_delivery = ActiveValue::Set(Some(estimated_delivery_date.clone()));
        }

        shipment.updated_at = ActiveValue::Set(Utc::now());

        shipment.update(db).await.map_err(|e| {
            error!("Failed to update shipment ID {}: {}", self.id, e);
            ServiceError::db_error(format!("Failed to update shipment: {}", e))
        })
    }

    async fn log_and_trigger_event(
        &self,
        event_sender: Arc<EventSender>,
        updated_shipment: &shipment::Model,
    ) -> Result<(), ServiceError> {
        info!("Shipment updated: {}", self.id);

        event_sender
            .send(Event::ShipmentUpdated(self.id))
            .await
            .map_err(|e| {
                error!(
                    "Failed to send ShipmentUpdated event for shipment ID {}: {}",
                    self.id, e
                );
                ServiceError::EventError(e.to_string())
            })
    }
}
