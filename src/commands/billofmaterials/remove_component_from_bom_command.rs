use uuid::Uuid;
use crate::{
    commands::Command,
    events::{Event, EventSender},
};
use crate::{
    db::DbPool,
    errors::ServiceError,
    models::{bom_line_item, prelude::*},
};
use async_trait::async_trait;
use sea_orm::{entity::*, query::*, DbConn, TransactionTrait};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info, instrument};
use validator::Validate;

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct RemoveComponentFromBOMCommand {
    pub bom_id: i32,
    pub component_id: i32, // ID of the component to remove
}

#[async_trait::async_trait]
impl Command for RemoveComponentFromBOMCommand {
    type Result = ();

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(
        &self,
        db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        let db = db_pool.clone();

        db.transaction::<_, ServiceError, _>(|txn| {
            Box::pin(async move {
                self.remove_component(txn).await?;
                Ok(())
            })
        })
        .await
        .map_err(|e| {
            error!(
                "Transaction failed for removing component {} from BOM ID {}: {}",
                self.component_id, self.bom_id, e
            );
            e
        })?;

        self.log_and_trigger_event(event_sender).await?;

        Ok(())
    }
}

impl RemoveComponentFromBOMCommand {
    async fn remove_component(&self, txn: &DbConn) -> Result<(), ServiceError> {
        bom_line_item::Entity::delete_many()
            .filter(bom_line_item::Column::BillOfMaterialsId.eq(self.bom_id))
            .filter(bom_line_item::Column::Id.eq(self.component_id))
            .exec(txn)
            .await
            .map_err(|e| {
                error!(
                    "Failed to remove component {} from BOM ID {}: {}",
                    self.component_id, self.bom_id, e
                );
                ServiceError::db_error(e)
            })?;
        Ok(())
    }

    async fn log_and_trigger_event(
        &self,
        event_sender: Arc<EventSender>,
    ) -> Result<(), ServiceError> {
        info!(
            "Component ID: {} removed from BOM ID: {}",
            self.component_id, self.bom_id
        );
        event_sender
            .send(Event::BOMCreated(self.bom_id))
            .await
            .map_err(|e| {
                error!("Failed to send event: {}", e);
                ServiceError::EventError(e.to_string())
            })?;
        Ok(())
    }
}
