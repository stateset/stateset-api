use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use crate::{errors::ServiceError, db::DbPool, models::{BOM}};
use crate::events::{Event, EventSender};
use tracing::{info, error, instrument};
use diesel::prelude::*;

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct UpdateBOMCommand {
    pub bom_id: i32,
    #[validate(length(min = 1))]
    pub name: Option<String>, // Optional new name for the BOM
    pub description: Option<String>, // Optional new description for the BOM
}

#[async_trait::async_trait]
impl Command for UpdateBOMCommand {
    type Result = BOM;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|e| {
            error!("Failed to get database connection: {}", e);
            ServiceError::DatabaseError("Failed to get database connection".into())
        })?;

        let updated_bom = conn.transaction(|| {
            self.update_bom(&conn)
        }).map_err(|e| {
            error!("Transaction failed for updating BOM ID {}: {}", self.bom_id, e);
            e
        })?;

        self.log_and_trigger_event(event_sender, &updated_bom).await?;

        Ok(updated_bom)
    }
}

impl UpdateBOMCommand {
    fn update_bom(&self, conn: &PgConnection) -> Result<BOM, ServiceError> {
        let target = boms::table.find(self.bom_id);

        diesel::update(target)
            .set((
                self.name.as_ref().map(|name| boms::name.eq(name)),
                self.description.as_ref().map(|desc| boms::description.eq(desc)),
            ))
            .get_result::<BOM>(conn)
            .map_err(|e| {
                error!("Failed to update BOM ID {}: {}", self.bom_id, e);
                ServiceError::DatabaseError(format!("Failed to update BOM: {}", e))
            })
    }

    async fn log_and_trigger_event(&self, event_sender: Arc<EventSender>, bom: &BOM) -> Result<(), ServiceError> {
        info!("BOM updated for BOM ID: {}", self.bom_id);
        event_sender.send(Event::BOMUpdated(bom.id))
            .await
            .map_err(|e| {
                error!("Failed to send BOMUpdated event for BOM ID {}: {}", bom.id, e);
                ServiceError::EventError(e.to_string())
            })
    }
}
