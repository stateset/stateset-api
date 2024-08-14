use std::sync::Arc;
use crate::{errors::ServiceError, db::DbPool, models::{Ticket, TicketComment, TicketStatus, TicketPriority}};
use crate::events::{Event, EventSender};
use validator::Validate;
use tracing::{info, error, instrument};
use diesel::prelude::*;
use serde::{Serialize, Deserialize};
use uuid::Uuid;
use chrono::Utc;

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct NewTicketInfo {
    #[validate(length(min = 1, max = 255))]
    pub title: String,
    #[validate(length(min = 1, max = 1000))]
    pub description: String,
    pub priority: TicketPriority,
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct SplitTicketCommand {
    pub original_ticket_id: Uuid,
    pub split_by: Uuid,
    #[validate]
    pub new_tickets: Vec<NewTicketInfo>,
    #[validate(length(min = 1, max = 1000))]
    pub split_reason: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SplitTicketResult {
    pub original_ticket_id: Uuid,
    pub new_ticket_ids: Vec<Uuid>,
    pub object: String,
    pub split: bool,
}

#[async_trait::async_trait]
impl Command for SplitTicketCommand {
    type Result = SplitTicketResult;

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

        let split_result = conn.transaction(|| {
            self.split_ticket(&conn)
        }).map_err(|e| {
            error!("Transaction failed for splitting ticket ID {}: {}", self.original_ticket_id, e);
            e
        })?;

        self.log_and_trigger_event(event_sender, &split_result).await?;

        Ok(SplitTicketResult {
            original_ticket_id: self.original_ticket_id,
            new_ticket_ids: split_result,
            object: "ticket".to_string(),
            split: true,
        })
    }
}

impl SplitTicketCommand {
    fn split_ticket(&self, conn: &PgConnection) -> Result<Vec<Uuid>, ServiceError> {
        use crate::schema::{tickets, ticket_comments};

        // Verify original ticket exists and is not closed
        let original_ticket: Ticket = tickets::table
            .find(self.original_ticket_id)
            .get_result::<Ticket>(conn)
            .map_err(|e| {
                if let diesel::result::Error::NotFound = e {
                    ServiceError::NotFound(format!("Ticket with ID {} not found", self.original_ticket_id))
                } else {
                    ServiceError::DatabaseError(format!("Failed to fetch original ticket: {}", e))
                }
            })?;

        if original_ticket.status == TicketStatus::Closed {
            return Err(ServiceError::ValidationError("Cannot split a closed ticket".into()));
        }

        // Create new tickets
        let mut new_ticket_ids = Vec::new();
        for new_ticket_info in &self.new_tickets {
            let new_ticket = Ticket {
                id: Uuid::new_v4(),
                title: new_ticket_info.title.clone(),
                description: new_ticket_info.description.clone(),
                status: TicketStatus::Open,
                priority: new_ticket_info.priority,
                user_id: original_ticket.user_id,  // Assuming we keep the same user
                assignee_id: None,  // New tickets are unassigned
                created_at: Utc::now(),
                updated_at: Utc::now(),
                closed_at: None,
                closed_by: None,
            };

            diesel::insert_into(tickets::table)
                .values(&new_ticket)
                .execute(conn)
                .map_err(|e| ServiceError::DatabaseError(format!("Failed to create new ticket: {}", e)))?;

            new_ticket_ids.push(new_ticket.id);

            // Add a comment to the new ticket about the split
            let split_comment = TicketComment {
                id: Uuid::new_v4(),
                ticket_id: new_ticket.id,
                user_id: self.split_by,
                content: format!("Created from split of ticket {}. Reason: {}", self.original_ticket_id, self.split_reason),
                created_at: Utc::now(),
            };

            diesel::insert_into(ticket_comments::table)
                .values(&split_comment)
                .execute(conn)
                .map_err(|e| ServiceError::DatabaseError(format!("Failed to add split comment: {}", e)))?;
        }

        // Update original ticket
        diesel::update(tickets::table.find(self.original_ticket_id))
            .set(tickets::updated_at.eq(Utc::now()))
            .execute(conn)
            .map_err(|e| ServiceError::DatabaseError(format!("Failed to update original ticket: {}", e)))?;

        // Add split comment to original ticket
        let original_split_comment = TicketComment {
            id: Uuid::new_v4(),
            ticket_id: self.original_ticket_id,
            user_id: self.split_by,
            content: format!("Ticket split into new tickets: {}. Reason: {}", new_ticket_ids.join(", "), self.split_reason),
            created_at: Utc::now(),
        };

        diesel::insert_into(ticket_comments::table)
            .values(&original_split_comment)
            .execute(conn)
            .map_err(|e| ServiceError::DatabaseError(format!("Failed to add split comment to original ticket: {}", e)))?;

        Ok(new_ticket_ids)
    }

    async fn log_and_trigger_event(&self, event_sender: Arc<EventSender>, new_ticket_ids: &[Uuid]) -> Result<(), ServiceError> {
        info!("Ticket split: Original ID {} into new IDs {:?} by user {}", 
              self.original_ticket_id, new_ticket_ids, self.split_by);
        event_sender.send(Event::TicketSplit(self.original_ticket_id, new_ticket_ids.to_vec(), self.split_by))
            .await
            .map_err(|e| {
                error!("Failed to send event for split ticket: {}", e);
                ServiceError::EventError(e.to_string())
            })
    }
}