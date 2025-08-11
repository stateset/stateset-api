use uuid::Uuid;
use crate::{
    commands::Command,
    events::{Event, EventSender},
    db::DbPool,
    errors::ServiceError,
    models::{shipment::{self, ShippingCarrier}, Shipment, ShipmentStatus},
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sea_orm::{ActiveValue::Set, ActiveModelTrait, EntityTrait, TransactionError, TransactionTrait};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info, instrument};
use validator::Validate;

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct CreateShipmentCommand {
    pub order_id: i32,  // Changed from Uuid to i32
    #[validate(length(min = 1))]
    pub shipping_address: String,
    pub shipping_method: shipment::ShippingMethod,
}

#[async_trait::async_trait]
impl Command for CreateShipmentCommand {
    type Result = shipment::Model;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(
        &self,
        db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        self.validate().map_err(|e| {
            error!("Validation failed: {:?}", e);
            ServiceError::ValidationError(e.to_string())
        })?;

        let db = db_pool.clone();

        let saved_shipment = db
            .transaction::<_, shipment::Model, ServiceError>(|txn| {
                Box::pin(async move {
                    let shipment = self.create_shipment(txn).await?;
                    Ok(shipment)
                })
            })
            .await
            .map_err(|e| {
                error!("Transaction failed for creating shipment: {}", e);
                match e {
                    TransactionError::Connection(db_err) => ServiceError::DatabaseError(db_err),
                    TransactionError::Transaction(service_err) => service_err,
                }
            })?;

        self.log_and_trigger_event(event_sender, &saved_shipment)
            .await?;

        Ok(saved_shipment)
    }
}

impl CreateShipmentCommand {
    async fn create_shipment(
        &self,
        txn: &sea_orm::DatabaseTransaction,
    ) -> Result<shipment::Model, ServiceError> {
        // Convert carrier string to ShippingCarrier enum if provided
        let carrier = match &self.carrier {
            Some(carrier_str) => match carrier_str.as_str() {
                "UPS" => shipment::ShippingCarrier::UPS,
                "FedEx" => shipment::ShippingCarrier::FedEx,
                "USPS" => shipment::ShippingCarrier::USPS,
                "DHL" => shipment::ShippingCarrier::DHL,
                _ => shipment::ShippingCarrier::Other,
            },
            None => shipment::ShippingCarrier::Other,
        };

        let tracking_number = self
            .tracking_number
            .clone()
            .unwrap_or_else(|| String::from("Pending"));

        let new_shipment = shipment::ActiveModel {
            order_id: Set(self.order_id),
            tracking_number: Set(self.generate_tracking_number()),
            carrier: Set(ShippingCarrier::Other), // Default carrier
            status: Set(ShipmentStatus::Pending),
            shipping_address: Set(self.shipping_address.clone()),
            shipping_method: Set(self.shipping_method.clone()),
            shipping_cost: Set(None), // Default cost
            weight_kg: Set(None), // Default weight
            estimated_delivery: Set(None),
            created_at: Set(Utc::now()),
            updated_at: Set(Utc::now()),
            ..Default::default()
        };

        new_shipment.insert(txn).await.map_err(|e| {
            error!("Failed to create shipment: {}", e);
            ServiceError::DatabaseError(e)
        })
    }

    async fn log_and_trigger_event(
        &self,
        event_sender: Arc<EventSender>,
        shipment: &shipment::Model,
    ) -> Result<(), ServiceError> {
        info!("Shipment created: {:?}", shipment);
        event_sender
            .send(Event::ShipmentCreated(Uuid::new_v4())) // Convert i32 to UUID for event
            .await
            .map_err(|e| {
                error!(
                    "Failed to send ShipmentCreated event for shipment ID {}: {}",
                    shipment.id, e
                );
                ServiceError::EventError(e.to_string())
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockall::mock;
    use mockall::predicate::*;
    use tokio::sync::broadcast;

    mock! {
        pub Database {}
        impl Clone for Database {
            fn clone(&self) -> Self;
        }
    }

    #[tokio::test]
    async fn test_validate_create_shipment_command() {
        // Test with valid data
        let valid_command = CreateShipmentCommand {
            order_id: uuid::Uuid::new_v4(),
            recipient_name: "John Doe".to_string(),
            shipping_address: "123 Main St, City, State 12345".to_string(),
            carrier: Some("FedEx".to_string()),
            tracking_number: Some("1234567890".to_string()),
        };

        assert!(valid_command.validate().is_ok());

        // Test with invalid data - empty recipient name
        let invalid_command1 = CreateShipmentCommand {
            order_id: uuid::Uuid::new_v4(),
            recipient_name: "".to_string(),
            shipping_address: "123 Main St, City, State 12345".to_string(),
            carrier: Some("FedEx".to_string()),
            tracking_number: Some("1234567890".to_string()),
        };

        assert!(invalid_command1.validate().is_err());

        // Test with invalid data - empty shipping address
        let invalid_command2 = CreateShipmentCommand {
            order_id: uuid::Uuid::new_v4(),
            recipient_name: "John Doe".to_string(),
            shipping_address: "".to_string(),
            carrier: Some("FedEx".to_string()),
            tracking_number: Some("1234567890".to_string()),
        };

        assert!(invalid_command2.validate().is_err());
    }
}
