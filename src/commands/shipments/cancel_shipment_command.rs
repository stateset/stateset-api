use crate::{
    commands::Command,
    db::DbPool,
    errors::ServiceError,
    events::{Event, EventSender},
    models::{
        shipment::{self, Entity as Shipment, ShipmentStatus},
        shipment_note,
    },
};
use sea_orm::{ActiveModelTrait, ActiveValue::Set, EntityTrait, TransactionTrait};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info, instrument};
use uuid::Uuid;
use validator::Validate;

#[derive(Debug, Serialize, Deserialize, Validate, Clone)]
pub struct CancelShipmentCommand {
    pub shipment_id: Uuid,

    #[validate(length(min = 1))]
    pub reason: String,
}

#[async_trait::async_trait]
impl Command for CancelShipmentCommand {
    type Result = shipment::Model;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(
        &self,
        db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        let db = db_pool.clone();

        let command_clone = self.clone();
        let updated_shipment = db
            .transaction::<_, shipment::Model, ServiceError>(move |txn| {
                let cmd = command_clone.clone();
                Box::pin(async move {
                    cmd.cancel_shipment(txn).await?;
                    cmd.log_cancellation_reason(txn).await?;
                    let payload = serde_json::json!({
                        "shipment_id": cmd.shipment_id.to_string(),
                        "reason": cmd.reason
                    });
                    let _ = crate::events::outbox::enqueue(
                        txn,
                        "shipment",
                        Some(cmd.shipment_id),
                        "ShipmentCancelled",
                        &payload,
                    )
                    .await;
                    let updated_shipment = cmd.fetch_updated_shipment(txn).await?;
                    Ok(updated_shipment)
                })
            })
            .await
            .map_err(|err| {
                error!(
                    "Transaction failed for cancelling shipment ID {}: {}",
                    self.shipment_id, err
                );
                match err {
                    sea_orm::TransactionError::Connection(db_err) => ServiceError::db_error(db_err),
                    sea_orm::TransactionError::Transaction(service_err) => service_err,
                }
            })?;

        self.log_and_trigger_event(event_sender, &updated_shipment)
            .await?;

        Ok(updated_shipment)
    }
}

impl CancelShipmentCommand {
    async fn cancel_shipment(
        &self,
        txn: &sea_orm::DatabaseTransaction,
    ) -> Result<(), ServiceError> {
        let mut shipment: shipment::ActiveModel = Shipment::find_by_id(self.shipment_id)
            .one(txn)
            .await
            .map_err(|e| {
                error!("Failed to find shipment: {}", e);
                ServiceError::db_error(e)
            })?
            .ok_or_else(|| {
                error!("Shipment ID {} not found", self.shipment_id);
                ServiceError::NotFound(format!("Shipment ID {} not found", self.shipment_id))
            })?
            .into();

        shipment.status = Set(ShipmentStatus::Cancelled);
        shipment.update(txn).await.map_err(|e| {
            error!("Failed to cancel shipment: {}", e);
            ServiceError::db_error(e)
        })?;
        Ok(())
    }

    async fn log_cancellation_reason(
        &self,
        txn: &sea_orm::DatabaseTransaction,
    ) -> Result<(), ServiceError> {
        let new_note = shipment_note::ActiveModel {
            shipment_id: Set(self.shipment_id),
            note: Set(self.reason.clone()),
            ..Default::default() // Fill in other necessary fields if needed
        };

        new_note.insert(txn).await.map_err(|e| {
            error!("Failed to log cancellation reason: {}", e);
            ServiceError::db_error(e)
        })?;
        Ok(())
    }

    async fn fetch_updated_shipment(
        &self,
        txn: &sea_orm::DatabaseTransaction,
    ) -> Result<shipment::Model, ServiceError> {
        Shipment::find_by_id(self.shipment_id)
            .one(txn)
            .await
            .map_err(|e| {
                error!("Failed to fetch updated shipment: {}", e);
                ServiceError::db_error(e)
            })?
            .ok_or_else(|| {
                error!("Shipment ID {} not found", self.shipment_id);
                ServiceError::NotFound(format!("Shipment ID {} not found", self.shipment_id))
            })
    }

    async fn log_and_trigger_event(
        &self,
        event_sender: Arc<EventSender>,
        updated_shipment: &shipment::Model,
    ) -> Result<(), ServiceError> {
        info!(
            "Shipment cancelled for shipment ID: {}. Reason: {}",
            self.shipment_id, self.reason
        );
        event_sender
            .send(Event::ShipmentCancelled(self.shipment_id))
            .await
            .map_err(|e| {
                error!(
                    "Failed to send ShipmentCancelled event for shipment ID {}: {}",
                    self.shipment_id, e
                );
                ServiceError::EventError(e.to_string())
            })
    }
}
