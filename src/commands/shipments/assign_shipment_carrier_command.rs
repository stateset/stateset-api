use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use crate::{errors::ServiceError, db::DbPool, models::{shipment, Shipment}};
use crate::events::{Event, EventSender};
use tracing::{info, error, instrument};
use sea_orm::{ActiveModelTrait, ActiveValue::Set, EntityTrait};
use validator::Validate;

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct AssignShipmentCarrierCommand {
    pub shipment_id: i32,
    
    #[validate(length(min = 1))]
    pub carrier_name: String, // Name of the shipping carrier
}

#[async_trait::async_trait]
impl Command for AssignShipmentCarrierCommand {
    type Result = shipment::Model;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        let db = db_pool.clone();

        let updated_shipment = self.assign_carrier(&db).await?;

        self.log_and_trigger_event(event_sender, &updated_shipment).await?;

        Ok(updated_shipment)
    }
}

impl AssignShipmentCarrierCommand {
    async fn assign_carrier(&self, db: &sea_orm::DatabaseConnection) -> Result<shipment::Model, ServiceError> {
        let shipment = shipment::Entity::find_by_id(self.shipment_id)
            .one(db)
            .await
            .map_err(|e| {
                error!("Failed to find shipment ID {}: {}", self.shipment_id, e);
                ServiceError::DatabaseError(format!("Failed to find shipment: {}", e))
            })?
            .ok_or_else(|| {
                error!("Shipment ID {} not found", self.shipment_id);
                ServiceError::NotFound
            })?;

        let mut shipment_active_model: shipment::ActiveModel = shipment.into();

        shipment_active_model.carrier = Set(self.carrier_name.clone());

        shipment_active_model
            .update(db)
            .await
            .map_err(|e| {
                error!("Failed to assign carrier to shipment ID {}: {}", self.shipment_id, e);
                ServiceError::DatabaseError(format!("Failed to assign carrier: {}", e))
            })
    }

    async fn log_and_trigger_event(&self, event_sender: Arc<EventSender>, shipment: &shipment::Model) -> Result<(), ServiceError> {
        info!("Carrier assigned to shipment ID: {}. Carrier: {}", self.shipment_id, self.carrier_name);
        event_sender.send(Event::CarrierAssignedToShipment(self.shipment_id, self.carrier_name.clone()))
            .await
            .map_err(|e| {
                error!("Failed to send CarrierAssignedToShipment event for shipment ID {}: {}", self.shipment_id, e);
                ServiceError::EventError(e.to_string())
            })
    }
}
