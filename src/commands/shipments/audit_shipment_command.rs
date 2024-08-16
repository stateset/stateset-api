use async_trait::async_trait;;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use crate::{errors::ServiceError, db::DbPool, models::shipment, models::Shipment};
use tracing::{info, error, instrument};
use sea_orm::{entity::*, query::*, ColumnTrait, EntityTrait};

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct AuditShipmentCommand {
    pub shipment_id: i32,
}

#[async_trait::async_trait]
impl Command for AuditShipmentCommand {
    type Result = Shipment;

    #[instrument(skip(self, db_pool))]
    async fn execute(&self, db_pool: Arc<DbPool>) -> Result<Self::Result, ServiceError> {
        let db = db_pool.clone();

        let shipment = self.audit_shipment(&db).await?;

        self.log_audit_result(&shipment);

        Ok(shipment)
    }
}

impl AuditShipmentCommand {
    async fn audit_shipment(&self, db: &sea_orm::DatabaseConnection) -> Result<Shipment, ServiceError> {
        shipment::Entity::find_by_id(self.shipment_id)
            .one(db)
            .await
            .map_err(|e| {
                error!("Failed to audit shipment with ID {}: {}", self.shipment_id, e);
                ServiceError::DatabaseError(format!("Failed to audit shipment: {}", e))
            })?
            .ok_or_else(|| {
                error!("Shipment with ID {} not found", self.shipment_id);
                ServiceError::NotFound
            })
    }

    fn log_audit_result(&self, shipment: &Shipment) {
        info!("Shipment audit completed for shipment ID: {}. Status: {:?}", shipment.id, shipment.status);
    }
}
