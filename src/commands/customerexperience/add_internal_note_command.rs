use std::sync::Arc;
use crate::{errors::ServiceError, db::DbPool, models::{Ticket, InternalNote}};
use crate::events::{Event, EventSender};
use validator::Validate;
use tracing::{info, error, instrument};
use diesel::prelude::*;
use serde::{Serialize, Deserialize};
use uuid::Uuid;
use chrono::Utc;

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct AddInternalNoteCommand {
    pub ticket_id: Uuid,
    pub author_id: Uuid,
    #[validate(length(min = 1, max = 10000))]
    pub content: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AddInternalNoteResult {
    pub ticket_id: Uuid,
    pub note_id: Uuid,
    pub object: String,
    pub note_added: bool,
}

#[async_trait::async_trait]
impl Command for AddInternalNoteCommand {
    type Result = AddInternalNoteResult;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        self.validate().map_err(|e| {
            error!("Validation error: {}", e);
            ServiceError::ValidationError(e.to_string())
        })?;

        let conn = db_pool.get().map_err(|e| {
            error!("Database connection error: {}", e);
            ServiceError::DatabaseError("Failed to get database connection".into())
        })?;

        let internal_note = conn.transaction(|| {
            self.add_internal_note(&conn)
        }).map_err(|e| {
            error!("Transaction failed for adding internal note to ticket ID {}: {}", self.ticket_id, e);
            e
        })?;

        self.log_and_trigger_event(event_sender, &internal_note).await?;

        Ok(AddInternalNoteResult {
            ticket_id: self.ticket_id,
            note_id: internal_note.id,
            object: "internal_note".to_string(),
            note_added: true,
        })
    }
}

impl AddInternalNoteCommand {
    fn add_internal_note(&self, conn: &PgConnection) -> Result<InternalNote, ServiceError> {
        use crate::schema::{tickets, internal_notes};

        // Verify ticket exists
        let ticket = tickets::table
            .find(self.ticket_id)
            .get_result::<Ticket>(conn)
            .map_err(|e| {
                if let diesel::result::Error::NotFound = e {
                    ServiceError::NotFound(format!("Ticket with ID {} not found", self.ticket_id))
                } else {
                    ServiceError::DatabaseError(format!("Failed to fetch ticket: {}", e))
                }
            })?;

        // Create new internal note
        let new_note = InternalNote {
            id: Uuid::new_v4(),
            ticket_id: self.ticket_id,
            author_id: self.author_id,
            content: self.content.clone(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        diesel::insert_into(internal_notes::table)
            .values(&new_note)
            .execute(conn)
            .map_err(|e| ServiceError::DatabaseError(format!("Failed to insert internal note: {}", e)))?;

        // Update the ticket's updated_at timestamp
        diesel::update(tickets::table.find(self.ticket_id))
            .set(tickets::updated_at.eq(Utc::now()))
            .execute(conn)
            .map_err(|e| ServiceError::DatabaseError(format!("Failed to update ticket timestamp: {}", e)))?;

        Ok(new_note)
    }

    async fn log_and_trigger_event(&self, event_sender: Arc<EventSender>, note: &InternalNote) -> Result<(), ServiceError> {
        info!("Internal note added to ticket ID {}: Note ID {}", self.ticket_id, note.id);
        event_sender.send(Event::InternalNoteAdded(self.ticket_id, note.id, self.author_id))
            .await
            .map_err(|e| {
                error!("Failed to send event for added internal note: {}", e);
                ServiceError::EventError(e.to_string())
            })
    }
}