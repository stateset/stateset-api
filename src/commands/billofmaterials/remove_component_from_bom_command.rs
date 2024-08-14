use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use crate::{errors::ServiceError, db::DbPool, models::BOMComponent};
use crate::events::{Event, EventSender};
use tracing::{info, error, instrument};
use diesel::prelude::*;

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct RemoveComponentFromBOMCommand {
    pub bom_id: i32,
    pub component_id: i32, // ID of the component to remove
}

#[async_trait::async_trait]
impl Command for RemoveComponentFromBOMCommand {
    type Result = ();

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|e| {
            error!("Failed to get database connection: {}", e);
            ServiceError::DatabaseError("Failed to get database connection".into())
        })?;

        conn.transaction(|| {
            self.remove_component(&conn)
        }).map_err(|e| {
            error!("Transaction failed for removing component {} from BOM ID {}: {}", self.component_id, self.bom_id, e);
            e
        })?;

        self.log_and_trigger_event(event_sender).await?;

        Ok(())
    }
}

impl RemoveComponentFromBOMCommand {
    fn remove_component(&self, conn: &PgConnection) -> Result<(), ServiceError> {
        diesel::delete(bom_components::table)
            .filter(bom_components::bom_id.eq(self.bom_id))
            .filter(bom_components::component_id.eq(self.component_id))
            .execute(conn)
            .map_err(|e| {
                error!("Failed to remove component {} from BOM ID {}: {}", self.component_id, self.bom_id, e);
                ServiceError::DatabaseError(format!("Failed to remove component: {}", e))
            })?;
        Ok(())
    }

    async fn log_and_trigger_event(&self, event_sender: Arc<EventSender>) -> Result<(), ServiceError> {
        info!("Component ID: {} removed from BOM ID: {}", self.component_id, self.bom_id);
        event_sender.send(Event::ComponentRemovedFromBOM(self.bom_id, self.component_id))
            .await
            .map_err(|e| {
                error!("Failed to send ComponentRemovedFromBOM event for BOM ID {}: {}", self.bom_id, e);
                ServiceError::EventError(e.to_string())
            })
    }
}
