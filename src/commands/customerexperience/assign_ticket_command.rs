use std::sync::Arc;
use crate::{errors::ServiceError, db::DbPool, models::Ticket};
use crate::events::{Event, EventSender};
use validator::Validate;
use tracing::{info, error, instrument};
use diesel::prelude::*;
use serde::{Serialize, Deserialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct AssignTicketCommand {
    pub ticket_id: Uuid,
    pub assignee_id: Uuid,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AssignTicketResult {
    pub id: Uuid,
    pub object: String,
    pub assigned: bool,
}

#[async_trait::async_trait]
impl Command for AssignTicketCommand {
    type Result = AssignTicketResult;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|e| {
            error!("Database connection error: {}", e);
            ServiceError::DatabaseError("Failed to get database connection".into())
        })?;

        let assigned_ticket = conn.transaction(|| {
            self.assign_ticket(&conn)
        }).map_err(|e| {
            error!("Transaction failed for assigning ticket ID {}: {}", self.ticket_id, e);
            e
        })?;

        self.log_and_trigger_event(event_sender, &assigned_ticket).await?;

        Ok(AssignTicketResult {
            id: assigned_ticket.id,
            object: "ticket".to_string(),
            assigned: true,
        })
    }
}

impl AssignTicketCommand {
    fn assign_ticket(&self, conn: &PgConnection) -> Result<Ticket, ServiceError> {
        use crate::schema::tickets;

        diesel::update(tickets::table.find(self.ticket_id))
            .set(tickets::assignee_id.eq(self.assignee_id))
            .get_result::<Ticket>(conn)
            .map_err(|e| {
                if let diesel::result::Error::NotFound = e {
                    error!("Ticket not found: {}", self.ticket_id);
                    ServiceError::NotFound(format!("Ticket with ID {} not found", self.ticket_id))
                } else {
                    error!("Failed to assign ticket: {}", e);
                    ServiceError::DatabaseError(format!("Database error: {}", e))
                }
            })
    }

    async fn log_and_trigger_event(&self, event_sender: Arc<EventSender>, assigned_ticket: &Ticket) -> Result<(), ServiceError> {
        info!("Ticket assigned: ID {} to assignee {}", self.ticket_id, self.assignee_id);
        event_sender.send(Event::TicketAssigned(self.ticket_id, self.assignee_id))
            .await
            .map_err(|e| {
                error!("Failed to send event for assigned ticket: {}", e);
                ServiceError::EventError(e.to_string())
            })
    }
}