use crate::{
    commands::Command,
    db::DbPool,
    entities::{work_order, work_order_note},
    errors::ServiceError,
    events::{Event, EventSender},
};
use async_trait::async_trait;
use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, DatabaseTransaction, EntityTrait, Set, TransactionError, TransactionTrait,
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
        let txn_command = self.clone();

        let work_order_note = db
            .transaction::<_, work_order_note::Model, ServiceError>(move |txn| {
                let cmd = txn_command.clone();
                Box::pin(async move { cmd.add_note_to_work_order(txn).await })
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
        &self,
        txn: &DatabaseTransaction,
    ) -> Result<work_order_note::Model, ServiceError> {
        work_order::Entity::find_by_id(self.work_order_id)
            .one(txn)
            .await
            .map_err(ServiceError::db_error)?
            .ok_or_else(|| {
                ServiceError::NotFound(format!("Work order {} not found", self.work_order_id))
            })?;

        let new_note = work_order_note::ActiveModel {
            id: Set(Uuid::new_v4()),
            work_order_id: Set(self.work_order_id),
            note: Set(self.note.clone()),
            created_at: Set(Utc::now()),
            created_by: Set(None),
        };

        new_note.insert(txn).await.map_err(|e| {
            error!(
                "Failed to add note to Work Order ID {}: {}",
                self.work_order_id, e
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
