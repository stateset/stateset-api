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
pub struct ReopenTicketCommand {
    pub ticket_id: Uuid,
    pub reopened_by: Uuid,
    #[validate(length(min = 1, max = 1000))]
    pub reopen_reason: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ReopenTicketResult {
    pub id: Uuid,
    pub object: String,
    pub reopened: bool,
}

#[async_trait::async_trait]
impl Command for ReopenTicketCommand {
    type Result = ReopenTicketResult;

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

        let reopened_ticket = conn.transaction(|| {
            self.reopen_ticket(&conn)
        }).map_err(|e| {
            error!("Transaction failed for reopening ticket ID {}: {}", self.ticket_id, e);
            e
        })?;

        self.log_and_trigger_event(event_sender, &reopened_ticket).await?;

        Ok(ReopenTicketResult {
            id: reopened_ticket.id,
            object: "ticket".to_string(),
            reopened: true,
        })
    }
}

impl ReopenTicketCommand {
    fn reopen_ticket(&self, conn: &PgConnection) -> Result<Ticket, ServiceError> {
        use crate::schema::tickets;
        use crate::schema::ticket_comments;

        // Check if the ticket is currently closed
        let current_ticket = tickets::table.find(self.ticket_id)
            .get_result::<Ticket>(conn)
            .map_err(|e| {
                if let diesel::result::Error::NotFound = e {
                    error!("Ticket not found: {}", self.ticket_id);
                    ServiceError::NotFound(format!("Ticket with ID {} not found", self.ticket_id))
                } else {
                    error!("Failed to fetch ticket: {}", e);
                    ServiceError::DatabaseError(format!("Database error: {}", e))
                }
            })?;

        if current_ticket.status != TicketStatus::Closed {
            return Err(ServiceError::ValidationError("Only closed tickets can be reopened".into()));
        }

        // Update ticket status to Open
        let updated_ticket = diesel::update(tickets::table.find(self.ticket_id))
            .set((
                tickets::status.eq(TicketStatus::Open),
                tickets::closed_at.eq::<Option<chrono::DateTime<Utc>>>(None),
                tickets::closed_by.eq::<Option<Uuid>>(None),
            ))
            .get_result::<Ticket>(conn)
            .map_err(|e| {
                error!("Failed to reopen ticket: {}", e);
                ServiceError::DatabaseError(format!("Database error: {}", e))
            })?;

        // Add reopen reason as a comment
        let reopen_comment = crate::models::TicketComment {
            id: Uuid::new_v4(),
            ticket_id: self.ticket_id,
            user_id: self.reopened_by,
            content: format!("Ticket reopened. Reason: {}", self.reopen_reason),
            created_at: Utc::now(),
        };

        diesel::insert_into(ticket_comments::table)
            .values(&reopen_comment)
            .execute(conn)
            .map_err(|e| {
                error!("Failed to add reopen comment: {}", e);
                ServiceError::DatabaseError(format!("Database error: {}", e))
            })?;

        Ok(updated_ticket)
    }

    async fn log_and_trigger_event(&self, event_sender: Arc<EventSender>, reopened_ticket: &Ticket) -> Result<(), ServiceError> {
        info!("Ticket reopened: ID {} by user {}", self.ticket_id, self.reopened_by);
        event_sender.send(Event::TicketReopened(self.ticket_id, self.reopened_by))
            .await
            .map_err(|e| {
                error!("Failed to send event for reopened ticket: {}", e);
                ServiceError::EventError(e.to_string())
            })
    }
}