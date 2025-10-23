use crate::{
    commands::Command, db::DbPool, errors::ServiceError, events::EventSender, models::shipment,
};
use sea_orm::EntityTrait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info, instrument};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
pub struct AuditShipmentCommand {
    pub shipment_id: Uuid,
}

#[async_trait::async_trait]
impl Command for AuditShipmentCommand {
    type Result = shipment::Model;

    #[instrument(skip(self, db_pool, _event_sender))]
    async fn execute(
        &self,
        db_pool: Arc<DbPool>,
        _event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
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
    ) -> Result<shipment::Model, ServiceError> {
        shipment::Entity::find_by_id(self.shipment_id)
            .one(db)
            .await
            .map_err(|e| {
                error!("Failed to audit shipment: {}", e);
                ServiceError::db_error(e)
            })?
            .ok_or_else(|| {
                error!("Shipment with ID {} not found", self.shipment_id);
                ServiceError::NotFound(format!("Shipment with ID {} not found", self.shipment_id))
            })
    }

    fn log_audit_result(&self, shipment: &shipment::Model) {
        info!(
            "Shipment audit completed for shipment ID: {}. Status: {:?}",
            shipment.id, shipment.status
        );
    }
}
