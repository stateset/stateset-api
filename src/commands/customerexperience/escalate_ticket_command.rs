use std::sync::Arc;
use crate::{errors::ServiceError, db::DbPool, models::{Ticket, TicketStatus, TicketPriority}};
use crate::events::{Event, EventSender};
use validator::Validate;
use tracing::{info, error, instrument};
use diesel::prelude::*;
use serde::{Serialize, Deserialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct EscalateTicketCommand {
    pub ticket_id: Uuid,
    pub escalated_by: Uuid,
    #[validate(length(min = 1, max = 1000))]
    pub reason: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EscalateTicketResult {
    pub id: Uuid,
    pub object: String,
    pub escalated: bool,
}

#[async_trait::async_trait]
impl Command for EscalateTicketCommand {
    type Result = EscalateTicketResult;

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

        let escalated_ticket = conn.transaction(|| {
            self.escalate_ticket(&conn)
        }).map_err(|e| {
            error!("Transaction failed for escalating ticket ID {}: {}", self.ticket_id, e);
            e
        })?;

        self.log_and_trigger_event(event_sender, &escalated_ticket).await?;

        Ok(EscalateTicketResult {
            id: escalated_ticket.id,
            object: "ticket".to_string(),
            escalated: true,
        })
    }
}

impl EscalateTicketCommand {
    fn escalate_ticket(&self, conn: &PgConnection) -> Result<Ticket, ServiceError> {
        use crate::schema::tickets;
        use crate::schema::ticket_comments;

        // Update ticket priority and status
        let updated_ticket = diesel::update(tickets::table.find(self.ticket_id))
            .set((
                tickets::priority.eq(TicketPriority::High),
                tickets::status.eq(TicketStatus::Escalated),
            ))
            .get_result::<Ticket>(conn)
            .map_err(|e| {
                if let diesel::result::Error::NotFound = e {
                    error!("Ticket not found: {}", self.ticket_id);
                    ServiceError::NotFound(format!("Ticket with ID {} not found", self.ticket_id))
                } else {
                    error!("Failed to escalate ticket: {}", e);
                    ServiceError::DatabaseError(format!("Database error: {}", e))
                }
            })?;

        // Add escalation reason as a comment
        let escalation_comment = crate::models::TicketComment {
            id: Uuid::new_v4(),
            ticket_id: self.ticket_id,
            user_id: self.escalated_by,
            content: format!("Ticket escalated. Reason: {}", self.reason),
            created_at: chrono::Utc::now(),
        };

        diesel::insert_into(ticket_comments::table)
            .values(&escalation_comment)
            .execute(conn)
            .map_err(|e| {
                error!("Failed to add escalation comment: {}", e);
                ServiceError::DatabaseError(format!("Database error: {}", e))
            })?;

        Ok(updated_ticket)
    }

    async fn log_and_trigger_event(&self, event_sender: Arc<EventSender>, escalated_ticket: &Ticket) -> Result<(), ServiceError> {
        info!("Ticket escalated: ID {} by user {}", self.ticket_id, self.escalated_by);
        event_sender.send(Event::TicketEscalated(self.ticket_id, self.escalated_by))
            .await
            .map_err(|e| {
                error!("Failed to send event for escalated ticket: {}", e);
                ServiceError::EventError(e.to_string())
            })
    }
}