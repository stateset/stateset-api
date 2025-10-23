use crate::{
    commands::Command,
    db::DbPool,
    errors::ServiceError,
    events::EventSender,
    models::return_note_entity::{self, Entity as ReturnNote},
};
use chrono::{DateTime, Utc};
use sea_orm::{Set, *};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, instrument};
use uuid::Uuid;
use validator::Validate;

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct AddReturnNoteCommand {
    pub return_id: Uuid,
    #[validate(length(min = 1, max = 1000))]
    pub note: String,
    pub created_by: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AddReturnNoteResult {
    pub return_id: Uuid,
    pub content: String,
    pub created_by: Option<String>,
    pub created_at: DateTime<Utc>,
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
        self.validate()
            .map_err(|e| ServiceError::ValidationError(e.to_string()))?;

        let db = db_pool.as_ref();
        let new_note = return_note_entity::ActiveModel {
            return_id: Set(self.return_id),
            note_type: Set(return_note_entity::ReturnNoteType::System),
            content: Set(self.note.clone()),
            created_by: Set(self
                .created_by
                .as_ref()
                .and_then(|s| Uuid::parse_str(s).ok())),
            is_visible_to_customer: Set(false),
            created_at: Set(Utc::now()),
            updated_at: Set(Utc::now()),
            ..Default::default()
        };

        new_note.insert(db).await.map_err(|e| {
            let msg = format!("Failed to create return note: {}", e);
            error!("{}", msg);
            ServiceError::db_error(e)
        })?;

        Ok(AddReturnNoteResult {
            return_id: self.return_id,
            content: self.note.clone(),
            created_by: self.created_by.clone(),
            created_at: Utc::now(),
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
            note_type: Set(return_note_entity::ReturnNoteType::System),
            content: Set(self.note.clone()),
            created_by: Set(self
                .created_by
                .as_ref()
                .and_then(|s| Uuid::parse_str(s).ok())),
            is_visible_to_customer: Set(false),
            created_at: Set(Utc::now()),
            updated_at: Set(Utc::now()),
            ..Default::default()
        };

        ReturnNote::insert(note.clone())
            .exec(db)
            .await
            .map_err(|e| {
                let msg = format!("Failed to add return note: {}", e);
                error!("{}", msg);
                ServiceError::db_error(sea_orm::DbErr::Custom(msg))
            })?;

        Ok(return_note_entity::Model {
            id: note_id,
            return_id: self.return_id,
            note_type: return_note_entity::ReturnNoteType::System,
            content: self.note.clone(),
            created_by: self
                .created_by
                .as_ref()
                .and_then(|s| Uuid::parse_str(s).ok()),
            is_visible_to_customer: false,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        })
    }
}
