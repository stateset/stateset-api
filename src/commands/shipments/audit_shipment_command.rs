use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use crate::{errors::ServiceError, db::DbPool, models::Shipment};
use crate::events::{Event, EventSender};
use tracing::{info, error, instrument};
use diesel::prelude::*;

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct AuditShipmentCommand {
    pub shipment_id: i32,
}

#[async_trait::async_trait]
impl Command for AuditShipmentCommand {
    type Result = Shipment;

    #[instrument(skip(self, db_pool))]
    async fn execute(&self, db_pool: Arc<DbPool>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|e| {
            error!("Failed to get database connection: {}", e);
            ServiceError::DatabaseError("Failed to get database connection".into())
        })?;

        let shipment = self.audit_shipment(&conn)?;

        self.log_audit_result(&shipment);

        Ok(shipment)
    }
}

impl AuditShipmentCommand {
    fn audit_shipment(&self, conn: &PgConnection) -> Result<Shipment, ServiceError> {
        shipments::table.find(self.shipment_id)
            .first::<Shipment>(conn)
            .map_err(|e| {
                error!("Failed to audit shipment with ID {}: {}", self.shipment_id, e);
                ServiceError::DatabaseError(format!("Failed to audit shipment: {}", e))
            })
    }

    fn log_audit_result(&self, shipment: &Shipment) {
        info!("Shipment audit completed for shipment ID: {}. Status: {:?}", shipment.id, shipment.status);
    }
}
