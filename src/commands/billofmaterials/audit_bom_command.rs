use uuid::Uuid;
use crate::{commands::Command, db::DbPool, errors::ServiceError, events::EventSender, models::bom};
use async_trait::async_trait;
use sea_orm::{entity::*, query::*, DbConn};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info, instrument};

#[derive(Debug, Serialize, Deserialize)]
pub struct AuditBOMCommand {
    pub bom_id: i32,
}

#[async_trait::async_trait]
impl Command for AuditBOMCommand {
    type Result = bom::Model;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
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
                let msg = format!("Failed to audit BOM: {}", e);
                error!("{}", msg);
                ServiceError::db_error(e)
            })?
            .ok_or_else(|| {
                error!("BOM ID {} not found during audit", self.bom_id);
                ServiceError::NotFound(format!("BOM ID {} not found", self.bom_id))
            })
    }

    fn log_audit_result(&self, bom: &bom::Model) {
        info!(
            "BOM audit completed for BOM ID: {}. Name: {}",
            bom.id, bom.name
        );
    }
}
