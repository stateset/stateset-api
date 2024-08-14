use std::sync::Arc;
use crate::{errors::ServiceError, db::DbPool, models::{Ticket, TicketStatus}};
use crate::events::{Event, EventSender};
use validator::Validate;
use tracing::{info, error, instrument};
use diesel::prelude::*;
use serde::{Serialize, Deserialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct UpdateTicketStatusCommand {
    pub ticket_id: Uuid,
    #[validate(custom = "validate_status")]
    pub new_status: TicketStatus,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateTicketStatusResult {
    pub id: Uuid,
    pub object: String,
    pub status_updated: bool,
}

#[async_trait::async_trait]
impl Command for UpdateTicketStatusCommand {
    type Result = UpdateTicketStatusResult;

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

        let updated_ticket = conn.transaction(|| {
            self.update_ticket_status(&conn)
        }).map_err(|e| {
            error!("Transaction failed for updating ticket status ID {}: {}", self.ticket_id, e);
            e
        })?;

        self.log_and_trigger_event(event_sender, &updated_ticket).await?;

        Ok(UpdateTicketStatusResult {
            id: updated_ticket.id,
            object: "ticket".to_string(),
            status_updated: true,
        })
    }
}

impl UpdateTicketStatusCommand {
    fn update_ticket_status(&self, conn: &PgConnection) -> Result<Ticket, ServiceError> {
        use crate::schema::tickets;

        diesel::update(tickets::table.find(self.ticket_id))
            .set(tickets::status.eq(self.new_status))
            .get_result::<Ticket>(conn)
            .map_err(|e| {
                if let diesel::result::Error::NotFound = e {
                    error!("Ticket not found: {}", self.ticket_id);
                    ServiceError::NotFound(format!("Ticket with ID {} not found", self.ticket_id))
                } else {
                    error!("Failed to update ticket status: {}", e);
                    ServiceError::DatabaseError(format!("Database error: {}", e))
                }
            })
    }

    async fn log_and_trigger_event(&self, event_sender: Arc<EventSender>, updated_ticket: &Ticket) -> Result<(), ServiceError> {
        info!("Ticket status updated: ID {} to status {:?}", self.ticket_id, self.new_status);
        event_sender.send(Event::TicketStatusUpdated(self.ticket_id, self.new_status))
            .await
            .map_err(|e| {
                error!("Failed to send event for updated ticket status: {}", e);
                ServiceError::EventError(e.to_string())
            })
    }
}

fn validate_status(status: &TicketStatus) -> Result<(), validator::ValidationError> {
    match status {
        TicketStatus::Open | TicketStatus::InProgress | TicketStatus::Resolved | TicketStatus::Closed => Ok(()),
        _ => Err(validator::ValidationError::new("Invalid ticket status")),
    }
}