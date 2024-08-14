use std::sync::Arc;
use crate::{errors::ServiceError, db::DbPool, models::{Ticket, TicketComment}};
use crate::events::{Event, EventSender};
use validator::Validate;
use tracing::{info, error, instrument};
use diesel::prelude::*;
use serde::{Serialize, Deserialize};
use uuid::Uuid;
use chrono::Utc;

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct AddTicketCommentCommand {
    pub ticket_id: Uuid,
    pub user_id: Uuid,
    #[validate(length(min = 1, max = 1000))]
    pub content: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AddTicketCommentResult {
    pub id: Uuid,
    pub object: String,
    pub comment_added: bool,
}

#[async_trait::async_trait]
impl Command for AddTicketCommentCommand {
    type Result = AddTicketCommentResult;

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

        let added_comment = conn.transaction(|| {
            self.add_ticket_comment(&conn)
        }).map_err(|e| {
            error!("Transaction failed for adding comment to ticket ID {}: {}", self.ticket_id, e);
            e
        })?;

        self.log_and_trigger_event(event_sender, &added_comment).await?;

        Ok(AddTicketCommentResult {
            id: added_comment.id,
            object: "ticket_comment".to_string(),
            comment_added: true,
        })
    }
}

impl AddTicketCommentCommand {
    fn add_ticket_comment(&self, conn: &PgConnection) -> Result<TicketComment, ServiceError> {
        use crate::schema::ticket_comments;

        let new_comment = TicketComment {
            id: Uuid::new_v4(),
            ticket_id: self.ticket_id,
            user_id: self.user_id,
            content: self.content.clone(),
            created_at: Utc::now(),
        };

        diesel::insert_into(ticket_comments::table)
            .values(&new_comment)
            .get_result::<TicketComment>(conn)
            .map_err(|e| {
                error!("Failed to add comment to ticket: {}", e);
                ServiceError::DatabaseError(format!("Database error: {}", e))
            })
    }

    async fn log_and_trigger_event(&self, event_sender: Arc<EventSender>, added_comment: &TicketComment) -> Result<(), ServiceError> {
        info!("Comment added to ticket: ID {} by user {}", self.ticket_id, self.user_id);
        event_sender.send(Event::TicketCommentAdded(added_comment.id, self.ticket_id))
            .await
            .map_err(|e| {
                error!("Failed to send event for added ticket comment: {}", e);
                ServiceError::EventError(e.to_string())
            })
    }
}