use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use crate::{errors::ServiceError, db::DbPool, models::BOM};
use crate::events::{Event, EventSender};
use tracing::{info, error, instrument};
use diesel::prelude::*;

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct DeleteBOMCommand {
    pub bom_id: i32,
}

#[async_trait::async_trait]
impl Command for DeleteBOMCommand {
    type Result = ();

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|e| {
            error!("Failed to get database connection: {}", e);
            ServiceError::DatabaseError("Failed to get database connection".into())
        })?;

        conn.transaction(|| {
            self.delete_bom(&conn)
        }).map_err(|e| {
            error!("Transaction failed for deleting BOM ID {}: {}", self.bom_id, e);
            e
        })?;

        self.log_and_trigger_event(event_sender).await?;

        Ok(())
    }
}

impl DeleteBOMCommand {
    fn delete_bom(&self, conn: &PgConnection) -> Result<(), ServiceError> {
        diesel::delete(boms::table.find(self.bom_id))
            .execute(conn)
            .map_err(|e| {
                error!("Failed to delete BOM ID {}: {}", self.bom_id, e);
                ServiceError::DatabaseError(format!("Failed to delete BOM: {}", e))
            })?;

        // Ensure all associated components are also deleted
        diesel::delete(bom_components::table.filter(bom_components::bom_id.eq(self.bom_id)))
            .execute(conn)
            .map_err(|e| {
                error!("Failed to delete components for BOM ID {}: {}", self.bom_id, e);
                ServiceError::DatabaseError(format!("Failed to delete BOM components: {}", e))
            })?;
        Ok(())
    }

    async fn log_and_trigger_event(&self, event_sender: Arc<EventSender>) -> Result<(), ServiceError> {
        info!("BOM ID: {} and its components were deleted.", self.bom_id);
        event_sender.send(Event::BOMDeleted(self.bom_id))
            .await
            .map_err(|e| {
                error!("Failed to send BOMDeleted event for BOM ID {}: {}", self.bom_id, e);
                ServiceError::EventError(e.to_string())
            })
    }
}
