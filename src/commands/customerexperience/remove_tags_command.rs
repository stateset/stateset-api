use std::sync::Arc;
use crate::{errors::ServiceError, db::DbPool, models::{Ticket, Tag, TicketTag}};
use crate::events::{Event, EventSender};
use validator::Validate;
use tracing::{info, error, instrument};
use diesel::prelude::*;
use serde::{Serialize, Deserialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct RemoveTagsFromTicketCommand {
    pub ticket_id: Uuid,
    #[validate(length(min = 1))]
    pub tags: Vec<String>,
    pub removed_by: Uuid,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RemoveTagsFromTicketResult {
    pub ticket_id: Uuid,
    pub removed_tags: Vec<String>,
    pub object: String,
    pub tags_removed: bool,
}

#[async_trait::async_trait]
impl Command for RemoveTagsFromTicketCommand {
    type Result = RemoveTagsFromTicketResult;

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

        let removed_tags = conn.transaction(|| {
            self.remove_tags(&conn)
        }).map_err(|e| {
            error!("Transaction failed for removing tags from ticket ID {}: {}", self.ticket_id, e);
            e
        })?;

        self.log_and_trigger_event(event_sender, &removed_tags).await?;

        Ok(RemoveTagsFromTicketResult {
            ticket_id: self.ticket_id,
            removed_tags,
            object: "ticket_tags".to_string(),
            tags_removed: true,
        })
    }
}

impl RemoveTagsFromTicketCommand {
    fn remove_tags(&self, conn: &PgConnection) -> Result<Vec<String>, ServiceError> {
        use crate::schema::{tickets, tags, ticket_tags};

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

        let mut removed_tags = Vec::new();

        for tag_name in &self.tags {
            // Find the tag
            if let Ok(tag) = tags::table
                .filter(tags::name.eq(tag_name))
                .first::<Tag>(conn)
            {
                // Remove the association between the ticket and the tag
                let deleted_count = diesel::delete(
                    ticket_tags::table
                        .filter(ticket_tags::ticket_id.eq(self.ticket_id))
                        .filter(ticket_tags::tag_id.eq(tag.id))
                )
                .execute(conn)?;

                if deleted_count > 0 {
                    removed_tags.push(tag_name.clone());
                }
            }
        }

        Ok(removed_tags)
    }

    async fn log_and_trigger_event(&self, event_sender: Arc<EventSender>, removed_tags: &[String]) -> Result<(), ServiceError> {
        info!("Tags removed from ticket ID {}: {:?}", self.ticket_id, removed_tags);
        event_sender.send(Event::TagsRemovedFromTicket(self.ticket_id, removed_tags.to_vec(), self.removed_by))
            .await
            .map_err(|e| {
                error!("Failed to send event for removed tags: {}", e);
                ServiceError::EventError(e.to_string())
            })
    }
}