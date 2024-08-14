use std::sync::Arc;
use crate::{errors::ServiceError, db::DbPool, models::{Ticket, TicketStatus}};
use crate::events::{Event, EventSender};
use validator::Validate;
use tracing::{info, error, instrument};
use diesel::prelude::*;
use serde::{Serialize, Deserialize};
use uuid::Uuid;
use chrono::Utc;

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct CloseTicketCommand {
    pub ticket_id: Uuid,
    pub closed_by: Uuid,
    #[validate(length(min = 1, max = 1000))]
    pub resolution_note: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CloseTicketResult {
    pub id: Uuid,
    pub object: String,
    pub closed: bool,
}

#[async_trait::async_trait]
impl Command for CloseTicketCommand {
    type Result = CloseTicketResult;

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

        let closed_ticket = conn.transaction(|| {
            self.close_ticket(&conn)
        }).map_err(|e| {
            error!("Transaction failed for closing ticket ID {}: {}", self.ticket_id, e);
            e
        })?;

        self.log_and_trigger_event(event_sender, &closed_ticket).await?;

        Ok(CloseTicketResult {
            id: closed_ticket.id,
            object: "ticket".to_string(),
            closed: true,
        })
    }
}

impl CloseTicketCommand {
    fn close_ticket(&self, conn: &PgConnection) -> Result<Ticket, ServiceError> {
        use crate::schema::tickets;
        use crate::schema::ticket_comments;

        // Update ticket status to Closed
        let updated_ticket = diesel::update(tickets::table.find(self.ticket_id))
            .set((
                tickets::status.eq(TicketStatus::Closed),
                tickets::closed_at.eq(Some(Utc::now())),
                tickets::closed_by.eq(Some(self.closed_by)),
            ))
            .get_result::<Ticket>(conn)
            .map_err(|e| {
                if let diesel::result::Error::NotFound = e {
                    error!("Ticket not found: {}", self.ticket_id);
                    ServiceError::NotFound(format!("Ticket with ID {} not found", self.ticket_id))
                } else {
                    error!("Failed to close ticket: {}", e);
                    ServiceError::DatabaseError(format!("Database error: {}", e))
                }
            })?;

        // Add resolution note as a comment
        let resolution_comment = crate::models::TicketComment {
            id: Uuid::new_v4(),
            ticket_id: self.ticket_id,
            user_id: self.closed_by,
            content: format!("Ticket closed. Resolution: {}", self.resolution_note),
            created_at: Utc::now(),
        };

        diesel::insert_into(ticket_comments::table)
            .values(&resolution_comment)
            .execute(conn)
            .map_err(|e| {
                error!("Failed to add resolution comment: {}", e);
                ServiceError::DatabaseError(format!("Database error: {}", e))
            })?;

        Ok(updated_ticket)
    }

    async fn log_and_trigger_event(&self, event_sender: Arc<EventSender>, closed_ticket: &Ticket) -> Result<(), ServiceError> {
        info!("Ticket closed: ID {} by user {}", self.ticket_id, self.closed_by);
        event_sender.send(Event::TicketClosed(self.ticket_id, self.closed_by))
            .await
            .map_err(|e| {
                error!("Failed to send event for closed ticket: {}", e);
                ServiceError::EventError(e.to_string())
            })
    }
}