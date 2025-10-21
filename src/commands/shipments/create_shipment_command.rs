use crate::{
    commands::Command,
    db::DbPool,
    errors::ServiceError,
    events::{Event, EventSender},
    models::{
        shipment::{self, ShippingCarrier},
        Shipment, ShipmentStatus,
    },
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, EntityTrait, TransactionError, TransactionTrait,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info, instrument};
use uuid::Uuid;
use validator::Validate;

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct CreateShipmentCommand {
    pub order_id: uuid::Uuid,
    #[validate(length(min = 1))]
    pub shipping_address: String,
    pub shipping_method: shipment::ShippingMethod,
    #[validate(length(min = 1))]
    pub tracking_number: String,
    #[validate(length(min = 1))]
    pub recipient_name: String,
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

        // Capture data to avoid borrowing across 'static closure
        let order_id = self.order_id;
        let shipping_address = self.shipping_address.clone();
        let shipping_method = self.shipping_method;
        let tracking_number = self.tracking_number.clone();
        let recipient_name = self.recipient_name.clone();

        let saved_shipment = db
            .transaction::<_, shipment::Model, ServiceError>(move |txn| {
                let shipping_address = shipping_address.clone();
                let tracking_number = tracking_number.clone();
                let recipient_name = recipient_name.clone();
                Box::pin(async move {
                    let new_shipment = shipment::ActiveModel {
                        id: Set(Uuid::nil()),
                        order_id: Set(order_id),
                        tracking_number: Set(tracking_number),
                        carrier: Set(ShippingCarrier::Other),
                        status: Set(ShipmentStatus::Pending),
                        shipping_address: Set(shipping_address),
                        shipping_method: Set(shipping_method),
                        weight_kg: Set(None),
                        dimensions_cm: Set(None),
                        notes: Set(None),
                        shipped_at: Set(None),
                        estimated_delivery: Set(None),
                        delivered_at: Set(None),
                        created_at: Set(Utc::now()),
                        updated_at: Set(Utc::now()),
                        created_by: Set(None),
                        recipient_name: Set(recipient_name),
                        recipient_email: Set(None),
                        recipient_phone: Set(None),
                        tracking_url: Set(None),
                        shipping_cost: Set(None),
                        insurance_amount: Set(None),
                        is_signature_required: Set(false),
                    };

                    let inserted = shipment::Entity::insert(new_shipment)
                        .exec_with_returning(txn)
                        .await
                        .map_err(ServiceError::db_error)?;
                    Ok(inserted)
                })
            })
            .await
            .map_err(|e| {
                error!("Transaction failed for creating shipment: {}", e);
                match e {
                    TransactionError::Connection(db_err) => ServiceError::db_error(db_err),
                    TransactionError::Transaction(service_err) => service_err,
                }
            })?;

        self.log_and_trigger_event(event_sender, &saved_shipment)
            .await?;

        Ok(saved_shipment)
    }
}

impl CreateShipmentCommand {
    async fn log_and_trigger_event(
        &self,
        event_sender: Arc<EventSender>,
        shipment: &shipment::Model,
    ) -> Result<(), ServiceError> {
        info!("Shipment created: {:?}", shipment);
        event_sender
            .send(Event::ShipmentCreated(shipment.id))
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

#[cfg(all(test, feature = "mock-tests"))]
mod tests {
    use super::*;

    #[test]
    fn validate_create_shipment_command() {
        let cmd = CreateShipmentCommand {
            order_id: uuid::Uuid::new_v4(),
            shipping_address: "123 Main St".to_string(),
            shipping_method: shipment::ShippingMethod::Standard,
            tracking_number: "TRACK123".to_string(),
            recipient_name: "John Doe".to_string(),
        };
        assert!(cmd.validate().is_ok());
    }
}
