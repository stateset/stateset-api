use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use crate::{errors::ServiceError, db::DbPool, models::{shipment, Shipment, ShipmentStatus, shipment_note, NewShipmentNote}};
use crate::events::{Event, EventSender};
use tracing::{info, error, instrument};
use sea_orm::{entity::*, query::*, ActiveValue::Set};

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct CancelShipmentCommand {
    pub shipment_id: i32,

    #[validate(length(min = 1))]
    pub reason: String,
}

#[async_trait::async_trait]
impl Command for CancelShipmentCommand {
    type Result = shipment::Model;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        let db = db_pool.clone();

        let updated_shipment = db
            .transaction::<_, ServiceError, _>(|txn| {
                Box::pin(async move {
                    self.cancel_shipment(txn).await?;
                    self.log_cancellation_reason(txn).await?;
                    let updated_shipment = self.fetch_updated_shipment(txn).await?;
                    Ok(updated_shipment)
                })
            })
            .await
            .map_err(|e| {
                error!("Transaction failed for cancelling shipment ID {}: {}", self.shipment_id, e);
                e
            })?;

        self.log_and_trigger_event(event_sender, &updated_shipment).await?;

        Ok(updated_shipment)
    }
}

impl CancelShipmentCommand {
    async fn cancel_shipment(&self, txn: &sea_orm::DatabaseTransaction) -> Result<(), ServiceError> {
        let mut shipment: shipment::ActiveModel = shipment::Entity::find_by_id(self.shipment_id)
            .one(txn)
            .await
            .map_err(|e| {
                error!("Failed to find shipment ID {}: {}", self.shipment_id, e);
                ServiceError::DatabaseError(format!("Failed to find shipment: {}", e))
            })?
            .ok_or_else(|| {
                error!("Shipment ID {} not found", self.shipment_id);
                ServiceError::NotFound
            })?
            .into();

        shipment.status = Set(ShipmentStatus::Cancelled);

        shipment.update(txn).await.map_err(|e| {
            error!("Failed to cancel shipment ID {}: {}", self.shipment_id, e);
            ServiceError::DatabaseError(format!("Failed to cancel shipment: {}", e))
        })?;
        Ok(())
    }

    async fn log_cancellation_reason(&self, txn: &sea_orm::DatabaseTransaction) -> Result<(), ServiceError> {
        let new_note = shipment_note::ActiveModel {
            shipment_id: Set(self.shipment_id),
            note: Set(self.reason.clone()),
            ..Default::default() // Fill in other necessary fields if needed
        };

        shipment_note::Entity::insert(new_note)
            .exec(txn)
            .await
            .map_err(|e| {
                error!("Failed to log cancellation reason for shipment ID {}: {}", self.shipment_id, e);
                ServiceError::DatabaseError(format!("Failed to log cancellation reason: {}", e))
            })?;
        Ok(())
    }

    async fn fetch_updated_shipment(&self, txn: &sea_orm::DatabaseTransaction) -> Result<shipment::Model, ServiceError> {
        shipment::Entity::find_by_id(self.shipment_id)
            .one(txn)
            .await
            .map_err(|e| {
                error!("Failed to fetch updated shipment for ID {}: {}", self.shipment_id, e);
                ServiceError::DatabaseError(format!("Failed to fetch updated shipment: {}", e))
            })?
            .ok_or_else(|| {
                error!("Shipment ID {} not found", self.shipment_id);
                ServiceError::NotFound
            })
    }

    async fn log_and_trigger_event(&self, event_sender: Arc<EventSender>, updated_shipment: &shipment::Model) -> Result<(), ServiceError> {
        info!("Shipment cancelled for shipment ID: {}. Reason: {}", self.shipment_id, self.reason);
        event_sender.send(Event::ShipmentCancelled(self.shipment_id))
            .await
            .map_err(|e| {
                error!("Failed to send ShipmentCancelled event for shipment ID {}: {}", self.shipment_id, e);
                ServiceError::EventError(e.to_string())
            })
    }
}
