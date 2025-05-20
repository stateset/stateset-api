use std::sync::Arc;
use sea_orm::*;
use crate::{
    db::DbPool,
    errors::ServiceError,
    events::{Event, EventSender},
    models::order_note_entity::{self, Entity as OrderNote},
};
use serde::{Deserialize, Serialize};
use tracing::{error, info, instrument};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
pub struct DeleteOrderNoteCommand {
    pub order_id: Uuid,
    pub note_id: i32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeleteOrderNoteResult {
    pub id: i32,
    pub deleted: bool,
}

#[async_trait::async_trait]
impl Command for DeleteOrderNoteCommand {
    type Result = DeleteOrderNoteResult;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(
        &self,
        db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        let db = db_pool.as_ref();

        self.delete_note(db).await?;

        self.log_and_trigger_event(&event_sender).await?;

        Ok(DeleteOrderNoteResult { id: self.note_id, deleted: true })
    }
}

impl DeleteOrderNoteCommand {
    async fn delete_note(&self, db: &DatabaseConnection) -> Result<(), ServiceError> {
        OrderNote::delete_by_id(self.note_id)
            .exec(db)
            .await
            .map_err(|e| {
                let msg = format!("Failed to delete order note {}: {}", self.note_id, e);
                error!("{}", msg);
                ServiceError::DatabaseError(msg)
            })?;
        Ok(())
    }

    async fn log_and_trigger_event(
        &self,
        event_sender: &EventSender,
    ) -> Result<(), ServiceError> {
        info!(order_id = %self.order_id, note_id = %self.note_id, "Order note deleted");
        event_sender
            .send(Event::OrderNoteDeleted(self.order_id, self.note_id))
            .await
            .map_err(|e| {
                let msg = format!("Failed to send event for deleted order note: {}", e);
                error!("{}", msg);
                ServiceError::EventError(msg)
            })
    }
}
