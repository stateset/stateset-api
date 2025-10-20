use uuid::Uuid;
use crate::{
    commands::Command,
    db::DbPool,
    errors::ServiceError,
    events::{Event, EventSender},
    models::{billofmaterials, bom_line_item},
};
use async_trait::async_trait;
use sea_orm::{entity::*, query::*, DbConn, DatabaseTransaction, TransactionTrait, TransactionError};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info, instrument};
use validator::Validate;

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct DeleteBOMCommand {
    pub bom_id: i32,
}

#[async_trait::async_trait]
impl Command for DeleteBOMCommand {
    type Result = ();

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(
        &self,
        db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        let db = db_pool.clone();

        db.transaction::<_, (), ServiceError>(|txn| {
                Box::pin(async move { self.delete_bom(txn).await })
            })
            .await
            .map_err(|e| {
                error!("Transaction failed for deleting BOM ID {}: {}", self.bom_id, e);
                match e {
                    TransactionError::Connection(db_err) => ServiceError::db_error(db_err),
                    TransactionError::Transaction(service_err) => service_err,
                }
            })?;

        self.log_and_trigger_event(event_sender).await?;

        Ok(())
    }
}

impl DeleteBOMCommand {
    async fn delete_bom(&self, txn: &DatabaseTransaction) -> Result<(), ServiceError> {
        // Delete BOM components associated with the BOM
        bom_line_item::Entity::delete_many()
            .filter(bom_line_item::Column::BillOfMaterialsId.eq(self.bom_id))
            .exec(txn)
            .await
            .map_err(|e| {
                error!("Failed to delete BOM components for BOM ID {}: {}", self.bom_id, e);
                ServiceError::db_error(e)
            })?;

        // Delete the BOM itself
        billofmaterials::Entity::delete_by_id(self.bom_id)
            .exec(txn)
            .await
            .map_err(|e| {
                error!("Failed to delete BOM ID {}: {}", self.bom_id, e);
                ServiceError::db_error(e)
            })?;

        Ok(())
    }

    async fn log_and_trigger_event(
        &self,
        event_sender: Arc<EventSender>,
    ) -> Result<(), ServiceError> {
        info!("BOM ID: {} and its components were deleted.", self.bom_id);
        event_sender
            .send(Event::BOMDeleted(self.bom_id))
            .await
            .map_err(|e| {
                error!(
                    "Failed to send BOMDeleted event for BOM ID {}: {}",
                    self.bom_id, e
                );
                ServiceError::EventError(e.to_string())
            })
    }
}
