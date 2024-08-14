use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use crate::{errors::ServiceError, db::DbPool, models::{BOM, BOMComponent}};
use tracing::{info, error, instrument};
use diesel::prelude::*;

#[derive(Debug, Serialize, Deserialize)]
pub struct AuditBOMCommand {
    pub bom_id: i32,
}

#[async_trait::async_trait]
impl Command for AuditBOMCommand {
    type Result = BOM;

    #[instrument(skip(self, db_pool))]
    async fn execute(&self, db_pool: Arc<DbPool>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|e| {
            error!("Failed to get database connection: {}", e);
            ServiceError::DatabaseError("Failed to get database connection".into())
        })?;

        let bom = self.audit_bom(&conn)?;

        self.log_audit_result(&bom);

        Ok(bom)
    }
}

impl AuditBOMCommand {
    fn audit_bom(&self, conn: &PgConnection) -> Result<BOM, ServiceError> {
        // Here, auditing would involve checking the BOM and its components' integrity.
        // For simplicity, we'll just fetch the BOM and assume the audit passes.
        boms::table.find(self.bom_id)
            .first::<BOM>(conn)
            .map_err(|e| {
                error!("Failed to audit BOM ID {}: {}", self.bom_id, e);
                ServiceError::DatabaseError(format!("Failed to audit BOM: {}", e))
            })
    }

    fn log_audit_result(&self, bom: &BOM) {
        info!("BOM audit completed for BOM ID: {}. Name: {}", bom.id, bom.name);
    }
}
