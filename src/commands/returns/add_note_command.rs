use std::sync::Arc;
use sea_orm::*;
use crate::{
    db::DbPool,
    errors::ServiceError,
    events::{Event, EventSender},
    models::{
        return_note_entity::{self, Entity as ReturnNote},
    },
    commands::Command,
};
use serde::{Deserialize, Serialize};
use tracing::{error, info, instrument};
use uuid::Uuid;
use validator::Validate;
use chrono::Utc;

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct AddReturnNoteCommand {
    pub return_id: Uuid,
    #[validate(length(min = 1, max = 1000))]
    pub note: String,
    pub created_by: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AddReturnNoteResult {
    pub id: Uuid,
    pub return_id: Uuid,
    pub note: String,
}

#[async_trait::async_trait]
impl Command for AddReturnNoteCommand {
    type Result = AddReturnNoteResult;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(
        &self,
        db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        self.validate().map_err(|e| ServiceError::ValidationError(e.to_string()))?;

        let db = db_pool.as_ref();
        let saved = self.add_note(db).await?;

        event_sender
            .send(Event::with_data(format!("return_note_added:{}", self.return_id)))
            .await
            .map_err(ServiceError::EventError)?;

        Ok(AddReturnNoteResult {
            id: saved.id,
            return_id: saved.return_id,
            note: saved.note,
        })
    }
}

impl AddReturnNoteCommand {
    async fn add_note(
        &self,
        db: &DatabaseConnection,
    ) -> Result<return_note_entity::Model, ServiceError> {
        let note_id = Uuid::new_v4();
        let note = return_note_entity::ActiveModel {
            id: Set(note_id),
            return_id: Set(self.return_id),
            note: Set(self.note.clone()),
            created_by: Set(self.created_by.clone()),
            created_at: Set(Utc::now().naive_utc()),
            ..Default::default()
        };

        ReturnNote::insert(note.clone())
            .exec(db)
            .await
            .map_err(|e| {
                let msg = format!("Failed to add return note: {}", e);
                error!("{}", msg);
                ServiceError::DatabaseError(msg)
            })?;

        Ok(return_note_entity::Model {
            id: note_id,
            return_id: self.return_id,
            note: self.note.clone(),
            created_by: self.created_by.clone(),
            created_at: Utc::now().naive_utc(),
        })
    }
}
