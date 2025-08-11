use uuid::Uuid;
use crate::events::{Event, EventSender};
use crate::{
    commands::Command,
    db::DbPool,
    errors::ServiceError,
    models::{work_order_note_entity, NewWorkOrderNote},
};
use async_trait::async_trait;
use chrono::Utc;
use sea_orm::{ActiveModelTrait, DatabaseConnection, EntityTrait, Set, TransactionError, TransactionTrait, DatabaseTransaction};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info, instrument};

#[derive(Debug, Serialize, Deserialize)]
pub struct AddNoteToWorkOrderCommand {
    pub work_order_id: Uuid,
    pub note: String, // Note to be added to the work order
}
#[async_trait::async_trait]
impl Command for AddNoteToWorkOrderCommand {
    type Result = work_order_note_entity::Model;
    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(
        &self,
        db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        let db = db_pool.clone();
        let work_order_note = db.transaction::<_, work_order_note_entity::Model, ServiceError>(|txn| {
            Box::pin(async move { self.add_note_to_work_order(txn).await })
        })
        .await
        .map_err(|e| {
            error!("Transaction failed for adding note to work order: {}", e);
            match e {
                TransactionError::Connection(db_err) => ServiceError::DatabaseError(db_err.to_string()),
                TransactionError::Transaction(service_err) => service_err,
            }
        })?;
        self.log_and_trigger_event(event_sender, &work_order_note)
            .await?;
        Ok(work_order_note)
    }
}

impl AddNoteToWorkOrderCommand {
    async fn add_note_to_work_order(
        &self,
        txn: &DatabaseTransaction,
    ) -> Result<work_order_note_entity::Model, ServiceError> {
        let new_note = work_order_note_entity::ActiveModel {
            work_order_id: Set(self.work_order_id),
            note: Set(self.note.clone()),
            created_at: Set(Utc::now()),
            ..Default::default()
        };
        new_note.insert(txn).await.map_err(|e| {
            error!(
                "Failed to add note to Work Order ID {}: {}",
                self.work_order_id, e
            );
            ServiceError::DatabaseError(format!("Failed to add note: {}", e))
        })
    }

    async fn log_and_trigger_event(
        &self,
        event_sender: Arc<EventSender>,
        note: &work_order_note_entity::Model,
    ) -> Result<(), ServiceError> {
        info!(
            "Note added to Work Order ID: {}. Note ID: {}",
            self.work_order_id, note.id
        );
        event_sender
            .send(Event::WorkOrderNoteAdded(self.work_order_id, note.id))
            .await
            .map_err(|e| {
                error!(
                    "Failed to send WorkOrderNoteAdded event for Work Order ID {}: {}",
                    self.work_order_id, e
                );
                ServiceError::EventError(e.to_string())
            })
    }
}
