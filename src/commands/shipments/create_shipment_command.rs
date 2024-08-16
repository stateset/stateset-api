use async_trait::async_trait;;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use crate::{errors::ServiceError, db::DbPool, models::{shipment, Shipment, ShipmentStatus, ShippingMethod}};
use crate::events::{Event, EventSender};
use validator::Validate;
use tracing::{info, error, instrument};
use sea_orm::{entity::*, query::*, ActiveValue::Set};

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct CreateShipmentCommand {
    pub order_id: i32,
    
    #[validate(length(min = 1))]
    pub shipping_address: String,

    #[validate(range(min = 1))]
    pub shipping_method: ShippingMethod, // Enum representing the shipping method
}

#[async_trait::async_trait]
impl Command for CreateShipmentCommand {
    type Result = shipment::Model;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        self.validate().map_err(|e| {
            error!("Validation failed: {:?}", e);
            ServiceError::ValidationError(e.to_string())
        })?;

        let db = db_pool.clone();

        let saved_shipment = db.transaction::<_, ServiceError, _>(|txn| {
            Box::pin(async move {
                let shipment = self.create_shipment(txn).await?;
                Ok(shipment)
            })
        }).await.map_err(|e| {
            error!("Transaction failed for creating shipment: {}", e);
            e
        })?;

        self.log_and_trigger_event(event_sender, &saved_shipment).await?;

        Ok(saved_shipment)
    }
}

impl CreateShipmentCommand {
    async fn create_shipment(&self, txn: &sea_orm::DatabaseTransaction) -> Result<shipment::Model, ServiceError> {
        let new_shipment = shipment::ActiveModel {
            order_id: Set(self.order_id),
            shipping_address: Set(self.shipping_address.clone()),
            shipping_method: Set(self.shipping_method.clone()),
            status: Set(ShipmentStatus::Pending),
            ..Default::default() // Additional fields like tracking number, timestamps, etc.
        };

        new_shipment.insert(txn).await.map_err(|e| {
            error!("Failed to create shipment: {}", e);
            ServiceError::DatabaseError(format!("Failed to create shipment: {}", e))
        })
    }

    async fn log_and_trigger_event(&self, event_sender: Arc<EventSender>, shipment: &shipment::Model) -> Result<(), ServiceError> {
        info!("Shipment created: {:?}", shipment);
        event_sender.send(Event::ShipmentCreated(shipment.id))
            .await
            .map_err(|e| {
                error!("Failed to send ShipmentCreated event for shipment ID {}: {}", shipment.id, e);
                ServiceError::EventError(e.to_string())
            })
    }
}
