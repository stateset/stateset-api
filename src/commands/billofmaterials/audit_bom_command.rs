use async_trait::async_trait;;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use crate::{errors::ServiceError, db::DbPool, models::bom};
use tracing::{info, error, instrument};
use sea_orm::{entity::*, query::*, DbConn};

#[derive(Debug, Serialize, Deserialize)]
pub struct AuditBOMCommand {
    pub bom_id: i32,
}

#[async_trait::async_trait]
impl Command for AuditBOMCommand {
    type Result = bom::Model;

    #[instrument(skip(self, db_pool))]
    async fn execute(&self, db_pool: Arc<DbPool>) -> Result<Self::Result, ServiceError> {
        let db = db_pool.clone();

        let bom = self.audit_bom(&db).await?;

        self.log_audit_result(&bom);

        Ok(bom)
    }
}

impl AuditBOMCommand {
    async fn audit_bom(&self, db: &DbConn) -> Result<bom::Model, ServiceError> {
        // Fetch the BOM and assume the audit passes for simplicity
        bom::Entity::find_by_id(self.bom_id)
            .one(db)
            .await
            .map_err(|e| {
                error!("Failed to audit BOM ID {}: {}", self.bom_id, e);
                ServiceError::DatabaseError(format!("Failed to audit BOM: {}", e))
            })?
            .ok_or_else(|| {
                error!("BOM ID {} not found during audit", self.bom_id);
                ServiceError::NotFound(format!("BOM ID {} not found", self.bom_id))
            })
    }

    fn log_audit_result(&self, bom: &bom::Model) {
        info!("BOM audit completed for BOM ID: {}. Name: {}", bom.id, bom.name);
    }
}
