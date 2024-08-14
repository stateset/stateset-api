use std::sync::Arc;
use crate::{errors::ServiceError, db::DbPool, models::{Ticket, TicketStatus, TicketPriority}};
use crate::events::{Event, EventSender};
use validator::Validate;
use tracing::{info, error, instrument};
use chrono::{DateTime, Utc};
use diesel::prelude::*;
use serde::{Serialize, Deserialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct CreateTicketCommand {
    #[validate(length(min = 1, max = 255))]
    pub title: String,
    #[validate(length(min = 1))]
    pub description: String,
    pub user_id: Uuid,
    #[validate(custom = "validate_priority")]
    pub priority: TicketPriority,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateTicketResult {
    pub id: Uuid,
    pub object: String,
    pub created: bool,
}

#[async_trait::async_trait]
impl Command for CreateTicketCommand {
    type Result = CreateTicketResult;

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

        let created_ticket = conn.transaction(|| {
            self.create_ticket(&conn)
        }).map_err(|e| {
            error!("Transaction failed for creating ticket: {}", e);
            e
        })?;

        self.log_and_trigger_event(event_sender, &created_ticket).await?;

        Ok(CreateTicketResult {
            id: created_ticket.id,
            object: "ticket".to_string(),
            created: true,
        })
    }
}

impl CreateTicketCommand {
    fn create_ticket(&self, conn: &PgConnection) -> Result<Ticket, ServiceError> {
        use crate::schema::tickets;

        let new_ticket = Ticket {
            id: Uuid::new_v4(),
            title: self.title.clone(),
            description: self.description.clone(),
            status: TicketStatus::Open,
            priority: self.priority,
            user_id: self.user_id,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        diesel::insert_into(tickets::table)
            .values(&new_ticket)
            .get_result::<Ticket>(conn)
            .map_err(|e| {
                error!("Failed to create ticket: {}", e);
                ServiceError::DatabaseError(format!("Database error: {}", e))
            })
    }

    async fn log_and_trigger_event(&self, event_sender: Arc<EventSender>, created_ticket: &Ticket) -> Result<(), ServiceError> {
        info!("Ticket created with ID: {}", created_ticket.id);
        event_sender.send(Event::TicketCreated(created_ticket.id))
            .await
            .map_err(|e| {
                error!("Failed to send event for created ticket: {}", e);
                ServiceError::EventError(e.to_string())
            })
    }
}

fn validate_priority(priority: &TicketPriority) -> Result<(), validator::ValidationError> {
    match priority {
        TicketPriority::Low | TicketPriority::Medium | TicketPriority::High | TicketPriority::Urgent => Ok(()),
        _ => Err(validator::ValidationError::new("Invalid priority")),
    }
}