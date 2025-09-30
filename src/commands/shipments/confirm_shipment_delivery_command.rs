use uuid::Uuid;
use crate::{
    commands::Command,
    events::{Event, EventSender},
    db::DbPool,
    errors::ServiceError,
    models::shipment::{self, Entity as Shipment, ShipmentStatus},
};
use async_trait::async_trait;
use chrono::Utc;
use sea_orm::{ActiveModelTrait, ActiveValue::Set, EntityTrait};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info, instrument};
use validator::Validate;

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct ConfirmShipmentDeliveryCommand {
    pub shipment_id: i32,
}

#[async_trait::async_trait]
impl Command for ConfirmShipmentDeliveryCommand {
    type Result = Shipment;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(
        &self,
        db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        let db = db_pool.clone();
        let updated_shipment = self.confirm_delivery(&db).await?;
        // Enqueue outbox (outside txn but within same request)
        let payload = serde_json::json!({
            "shipment_id": updated_shipment.id.to_string(),
            "delivered_at": updated_shipment.delivered_at.map(|t| t.to_rfc3339()),
        });
        let _ = crate::events::outbox::enqueue(&*db, "shipment", Some(updated_shipment.id), "ShipmentDelivered", &payload).await;

        self.log_and_trigger_event(event_sender, &updated_shipment)
            .await?;

        Ok(updated_shipment)
    }
}

impl ConfirmShipmentDeliveryCommand {
    async fn confirm_delivery(
        &self,
        db: &sea_orm::DatabaseConnection,
    ) -> Result<Shipment, ServiceError> {
        let mut shipment: shipment::ActiveModel = shipment::Entity::find_by_id(self.shipment_id)
            .one(db)
            .await
            .map_err(|e| {
                error!("Failed to fetch shipment: {}", e);
                ServiceError::DatabaseError(e)
            })?
            .ok_or_else(|| {
                error!("Shipment with ID {} not found", self.shipment_id);
                ServiceError::NotFound(format!("Shipment with ID {} not found", self.shipment_id))
            })?
            .into();

        shipment.status = Set(ShipmentStatus::Delivered);
        shipment.delivered_at = Set(Some(Utc::now()));
        shipment.update(db).await.map_err(|e| {
            error!("Failed to confirm delivery: {}", e);
            ServiceError::DatabaseError(e)
        })
    }

    async fn log_and_trigger_event(
        &self,
        event_sender: Arc<EventSender>,
        shipment: &Shipment,
    ) -> Result<(), ServiceError> {
        info!("Shipment ID: {} confirmed as delivered.", self.shipment_id);
        event_sender
            .send(Event::ShipmentDelivered(self.shipment_id))
            .await
            .map_err(|e| {
                error!(
                    "Failed to send ShipmentDelivered event for shipment ID {}: {}",
                    self.shipment_id, e
                );
                ServiceError::EventError(e.to_string())
            })
    }
}
