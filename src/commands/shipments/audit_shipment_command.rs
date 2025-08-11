use uuid::Uuid;
use crate::{
    commands::Command,
    db::DbPool,
    errors::ServiceError,
    events::{Event, EventSender},
    models::shipment::{self, Entity as Shipment},
};
use async_trait::async_trait;
use sea_orm::EntityTrait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info, instrument};
use validator::Validate;

#[derive(Debug, Serialize, Deserialize)]
pub struct AuditShipmentCommand {
    pub shipment_id: i32,
}

#[async_trait::async_trait]
impl Command for AuditShipmentCommand {
    type Result = Shipment;

    #[instrument(skip(self, db_pool, _event_sender))]
    async fn execute(&self, db_pool: Arc<DbPool>, _event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        let db = db_pool.clone();

        let shipment = self.audit_shipment(&db).await?;

        self.log_audit_result(&shipment);

        Ok(shipment)
    }
}

impl AuditShipmentCommand {
    async fn audit_shipment(
        &self,
        db: &sea_orm::DatabaseConnection,
    ) -> Result<Shipment, ServiceError> {
        shipment::Entity::find_by_id(self.shipment_id)
            .one(db)
            .await
            .map_err(|e| {
                error!("Failed to audit shipment: {}", e);
                ServiceError::DatabaseError(e)
            })?
            .ok_or_else(|| {
                error!("Shipment with ID {} not found", self.shipment_id);
                ServiceError::NotFound(format!("Shipment with ID {} not found", self.shipment_id))
            })
    }

    fn log_audit_result(&self, shipment: &Shipment) {
        info!(
            "Shipment audit completed for shipment ID: {}. Status: {:?}",
            shipment.id, shipment.status
        );
    }
}
