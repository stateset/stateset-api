use crate::{
    commands::Command,
    db::DbPool,
    entities::{work_order, work_order_note},
    errors::ServiceError,
    events::{Event, EventSender},
};
use sea_orm::{
    ActiveModelTrait, DatabaseTransaction, EntityTrait, TransactionError, TransactionTrait,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info, instrument};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddNoteToWorkOrderCommand {
    pub work_order_id: Uuid,
    pub note: String,
}

#[async_trait::async_trait]
impl Command for AddNoteToWorkOrderCommand {
    type Result = work_order_note::Model;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(
        &self,
        db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        self.validate_inputs()?;

        let db = db_pool.clone();
        let work_order_id = self.work_order_id;
        let note_text = self.note.clone();

        let work_order_note = db
            .transaction::<_, work_order_note::Model, ServiceError>(move |txn| {
                Box::pin(async move {
                    Self::add_note_to_work_order(txn, work_order_id, note_text).await
                })
            })
            .await
            .map_err(|e| {
                error!("Transaction failed for adding note to work order: {}", e);
                match e {
                    TransactionError::Connection(db_err) => ServiceError::db_error(db_err),
                    TransactionError::Transaction(service_err) => service_err,
                }
            })?;

        self.clone()
            .log_and_trigger_event(event_sender, &work_order_note)
            .await?;

        Ok(work_order_note)
    }
}

impl AddNoteToWorkOrderCommand {
    fn validate_inputs(&self) -> Result<(), ServiceError> {
        if self.note.trim().is_empty() {
            return Err(ServiceError::ValidationError(
                "Note cannot be empty".to_string(),
            ));
        }
        Ok(())
    }

    async fn add_note_to_work_order(
        txn: &DatabaseTransaction,
        work_order_id: Uuid,
        note: String,
    ) -> Result<work_order_note::Model, ServiceError> {
        work_order::Entity::find_by_id(work_order_id)
            .one(txn)
            .await
            .map_err(ServiceError::db_error)?
            .ok_or_else(|| {
                ServiceError::NotFound(format!("Work order {} not found", work_order_id))
            })?;

        let model = work_order_note::Model::new(work_order_id, note, None).map_err(|e| {
            error!(
                "Validation failed when creating note for work order {}: {}",
                work_order_id, e
            );
            ServiceError::ValidationError("Invalid work order note input".to_string())
        })?;

        let new_note: work_order_note::ActiveModel = model.into();

        new_note.insert(txn).await.map_err(|e| {
            error!(
                "Failed to add note to Work Order ID {}: {}",
                work_order_id, e
            );
            ServiceError::db_error(e)
        })
    }

    async fn log_and_trigger_event(
        &self,
        event_sender: Arc<EventSender>,
        note: &work_order_note::Model,
    ) -> Result<(), ServiceError> {
        info!(
            "Note added to Work Order ID: {}. Note ID: {}",
            self.work_order_id, note.id
        );
        event_sender
            .send(Event::WorkOrderUpdated(self.work_order_id))
            .await
            .map_err(|e| {
                error!(
                    "Failed to send WorkOrderUpdated event for Work Order ID {}: {}",
                    self.work_order_id, e
                );
                ServiceError::EventError(e.to_string())
            })
    }
}
