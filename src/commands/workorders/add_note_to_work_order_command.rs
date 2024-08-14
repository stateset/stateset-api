use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use crate::{errors::ServiceError, db::DbPool, models::{WorkOrderNote, NewWorkOrderNote}};
use crate::events::{Event, EventSender};
use tracing::{info, error, instrument};
use diesel::prelude::*;
use chrono::Utc;

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct AddNoteToWorkOrderCommand {
    pub work_order_id: i32,
    #[validate(length(min = 1))]
    pub note: String, // Note to be added to the work order
}

#[async_trait::async_trait]
impl Command for AddNoteToWorkOrderCommand {
    type Result = WorkOrderNote;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|e| {
            error!("Failed to get database connection: {}", e);
            ServiceError::DatabaseError("Failed to get database connection".into())
        })?;

        let work_order_note = conn.transaction(|| {
            self.add_note_to_work_order(&conn)
        }).map_err(|e| {
            error!("Transaction failed for adding note to Work Order ID {}: {}", self.work_order_id, e);
            e
        })?;

        self.log_and_trigger_event(event_sender, &work_order_note).await?;

        Ok(work_order_note)
    }
}

impl AddNoteToWorkOrderCommand {
    fn add_note_to_work_order(&self, conn: &PgConnection) -> Result<WorkOrderNote, ServiceError> {
        let new_note = NewWorkOrderNote {
            work_order_id: self.work_order_id,
            note: self.note.clone(),
            created_at: Utc::now(),
        };

        diesel::insert_into(work_order_notes::table)
            .values(&new_note)
            .get_result::<WorkOrderNote>(conn)
            .map_err(|e| {
                error!("Failed to add note to Work Order ID {}: {}", self.work_order_id, e);
                ServiceError::DatabaseError(format!("Failed to add note: {}", e))
            })
    }

    async fn log_and_trigger_event(&self, event_sender: Arc<EventSender>, note: &WorkOrderNote) -> Result<(), ServiceError> {
        info!("Note added to Work Order ID: {}. Note ID: {}", self.work_order_id, note.id);
        event_sender.send(Event::WorkOrderNoteAdded(self.work_order_id, note.id))
            .await
            .map_err(|e| {
                error!("Failed to send WorkOrderNoteAdded event for Work Order ID {}: {}", self.work_order_id, e);
                ServiceError::EventError(e.to_string())
            })
    }
}
