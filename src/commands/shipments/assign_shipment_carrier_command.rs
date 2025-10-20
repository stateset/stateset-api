use crate::{
    commands::Command,
    db::DbPool,
    errors::ServiceError,
    events::{Event, EventSender},
    models::shipment::{self, Entity as Shipment, ShippingCarrier},
};
use async_trait::async_trait;
use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, QueryFilter};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info, instrument};
use uuid::Uuid;
use validator::Validate;

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct AssignShipmentCarrierCommand {
    pub shipment_id: Uuid,

    #[validate(length(min = 1))]
    pub carrier_name: String, // Name of the shipping carrier
}

#[async_trait::async_trait]
impl Command for AssignShipmentCarrierCommand {
    type Result = shipment::Model;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(
        &self,
        db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        let db = db_pool.clone();
        let updated_shipment = self.assign_carrier(&db).await?;
        // Outbox enqueue (string id)
        let payload = serde_json::json!({
            "shipment_id": self.shipment_id.to_string(),
            "carrier": self.carrier_name
        });
        let _ = crate::events::outbox::enqueue(
            &*db,
            "shipment",
            None,
            "CarrierAssignedToShipment",
            &payload,
        )
        .await;

        self.log_and_trigger_event(event_sender, &updated_shipment)
            .await?;

        Ok(updated_shipment)
    }
}

impl AssignShipmentCarrierCommand {
    async fn assign_carrier(
        &self,
        db: &sea_orm::DatabaseConnection,
    ) -> Result<shipment::Model, ServiceError> {
        let shipment = shipment::Entity::find_by_id(self.shipment_id)
            .one(db)
            .await
            .map_err(|e| {
                error!("Failed to find shipment: {}", e);
                ServiceError::db_error(e)
            })?
            .ok_or_else(|| {
                error!("Shipment ID {} not found", self.shipment_id);
                ServiceError::NotFound(format!("Shipment ID {} not found", self.shipment_id))
            })?;

        let mut shipment_active_model: shipment::ActiveModel = shipment.into();
        // Parse carrier name to ShippingCarrier enum
        let carrier = match self.carrier_name.to_lowercase().as_str() {
            "ups" => ShippingCarrier::UPS,
            "fedex" => ShippingCarrier::FedEx,
            "usps" => ShippingCarrier::USPS,
            "dhl" => ShippingCarrier::DHL,
            _ => ShippingCarrier::Other,
        };
        shipment_active_model.carrier = Set(carrier);

        let updated_shipment = shipment_active_model.update(db).await.map_err(|e| {
            error!("Failed to assign carrier: {}", e);
            ServiceError::db_error(e)
        })?;

        Ok(updated_shipment)
    }

    async fn log_and_trigger_event(
        &self,
        event_sender: Arc<EventSender>,
        shipment: &shipment::Model,
    ) -> Result<(), ServiceError> {
        info!(
            "Carrier assigned to shipment ID: {}. Carrier: {}",
            self.shipment_id, self.carrier_name
        );
        event_sender
            .send(Event::CarrierAssignedToShipment(
                shipment.id,
                self.carrier_name.clone(),
            ))
            .await
            .map_err(|e| {
                error!(
                    "Failed to send CarrierAssignedToShipment event for shipment ID {}: {}",
                    self.shipment_id, e
                );
                ServiceError::EventError(e.to_string())
            })
    }
}
