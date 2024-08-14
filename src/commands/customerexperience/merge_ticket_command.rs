use std::sync::Arc;
use crate::{errors::ServiceError, db::DbPool, models::{Ticket, TicketComment, TicketStatus}};
use crate::events::{Event, EventSender};
use validator::Validate;
use tracing::{info, error, instrument};
use diesel::prelude::*;
use serde::{Serialize, Deserialize};
use uuid::Uuid;
use chrono::Utc;

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct MergeTicketsCommand {
    pub primary_ticket_id: Uuid,
    pub secondary_ticket_ids: Vec<Uuid>,
    pub merged_by: Uuid,
    #[validate(length(min = 1, max = 1000))]
    pub merge_reason: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MergeTicketsResult {
    pub primary_ticket_id: Uuid,
    pub merged_ticket_ids: Vec<Uuid>,
    pub object: String,
    pub merged: bool,
}

#[async_trait::async_trait]
impl Command for MergeTicketsCommand {
    type Result = MergeTicketsResult;

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

        let merge_result = conn.transaction(|| {
            self.merge_tickets(&conn)
        }).map_err(|e| {
            error!("Transaction failed for merging tickets: {}", e);
            e
        })?;

        self.log_and_trigger_event(event_sender, &merge_result).await?;

        Ok(MergeTicketsResult {
            primary_ticket_id: self.primary_ticket_id,
            merged_ticket_ids: self.secondary_ticket_ids.clone(),
            object: "ticket".to_string(),
            merged: true,
        })
    }
}

impl MergeTicketsCommand {
    fn merge_tickets(&self, conn: &PgConnection) -> Result<Ticket, ServiceError> {
        use crate::schema::{tickets, ticket_comments};

        // Verify all tickets exist and are not already closed
        let mut all_ticket_ids = vec![self.primary_ticket_id];
        all_ticket_ids.extend(&self.secondary_ticket_ids);

        let existing_tickets: Vec<Ticket> = tickets::table
            .filter(tickets::id.eq_any(&all_ticket_ids))
            .load::<Ticket>(conn)
            .map_err(|e| ServiceError::DatabaseError(format!("Failed to fetch tickets: {}", e)))?;

        if existing_tickets.len() != all_ticket_ids.len() {
            return Err(ServiceError::ValidationError("One or more tickets not found".into()));
        }

        if existing_tickets.iter().any(|t| t.status == TicketStatus::Closed) {
            return Err(ServiceError::ValidationError("Cannot merge closed tickets".into()));
        }

        // Update primary ticket
        let primary_ticket = diesel::update(tickets::table.find(self.primary_ticket_id))
            .set(tickets::updated_at.eq(Utc::now()))
            .get_result::<Ticket>(conn)
            .map_err(|e| ServiceError::DatabaseError(format!("Failed to update primary ticket: {}", e)))?;

        // Move comments from secondary tickets to primary ticket
        for secondary_id in &self.secondary_ticket_ids {
            let comments: Vec<TicketComment> = TicketComment::belonging_to(&Ticket { id: *secondary_id, ..Default::default() })
                .load::<TicketComment>(conn)
                .map_err(|e| ServiceError::DatabaseError(format!("Failed to fetch comments: {}", e)))?;

            for mut comment in comments {
                comment.ticket_id = self.primary_ticket_id;
                diesel::insert_into(ticket_comments::table)
                    .values(&comment)
                    .execute(conn)
                    .map_err(|e| ServiceError::DatabaseError(format!("Failed to move comment: {}", e)))?;
            }
        }

        // Add merge comment to primary ticket
        let merge_comment = TicketComment {
            id: Uuid::new_v4(),
            ticket_id: self.primary_ticket_id,
            user_id: self.merged_by,
            content: format!("Merged tickets: {}. Reason: {}", self.secondary_ticket_ids.join(", "), self.merge_reason),
            created_at: Utc::now(),
        };

        diesel::insert_into(ticket_comments::table)
            .values(&merge_comment)
            .execute(conn)
            .map_err(|e| ServiceError::DatabaseError(format!("Failed to add merge comment: {}", e)))?;

        // Close secondary tickets
        diesel::update(tickets::table.filter(tickets::id.eq_any(&self.secondary_ticket_ids)))
            .set((
                tickets::status.eq(TicketStatus::Closed),
                tickets::closed_at.eq(Some(Utc::now())),
                tickets::closed_by.eq(Some(self.merged_by)),
            ))
            .execute(conn)
            .map_err(|e| ServiceError::DatabaseError(format!("Failed to close secondary tickets: {}", e)))?;

        Ok(primary_ticket)
    }

    async fn log_and_trigger_event(&self, event_sender: Arc<EventSender>, merged_ticket: &Ticket) -> Result<(), ServiceError> {
        info!("Tickets merged: Primary ID {} with Secondary IDs {:?} by user {}", 
              self.primary_ticket_id, self.secondary_ticket_ids, self.merged_by);
        event_sender.send(Event::TicketsMerged(self.primary_ticket_id, self.secondary_ticket_ids.clone(), self.merged_by))
            .await
            .map_err(|e| {
                error!("Failed to send event for merged tickets: {}", e);
                ServiceError::EventError(e.to_string())
            })
    }
}