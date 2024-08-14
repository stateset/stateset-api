use std::sync::Arc;
use crate::{errors::ServiceError, db::DbPool, models::{Ticket, Tag, TicketTag}};
use crate::events::{Event, EventSender};
use validator::Validate;
use tracing::{info, error, instrument};
use diesel::prelude::*;
use serde::{Serialize, Deserialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct AddTagsToTicketCommand {
    pub ticket_id: Uuid,
    #[validate(length(min = 1))]
    pub tags: Vec<String>,
    pub added_by: Uuid,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AddTagsToTicketResult {
    pub ticket_id: Uuid,
    pub added_tags: Vec<String>,
    pub object: String,
    pub tags_added: bool,
}

#[async_trait::async_trait]
impl Command for AddTagsToTicketCommand {
    type Result = AddTagsToTicketResult;

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

        let added_tags = conn.transaction(|| {
            self.add_tags(&conn)
        }).map_err(|e| {
            error!("Transaction failed for adding tags to ticket ID {}: {}", self.ticket_id, e);
            e
        })?;

        self.log_and_trigger_event(event_sender, &added_tags).await?;

        Ok(AddTagsToTicketResult {
            ticket_id: self.ticket_id,
            added_tags,
            object: "ticket_tags".to_string(),
            tags_added: true,
        })
    }
}

impl AddTagsToTicketCommand {
    fn add_tags(&self, conn: &PgConnection) -> Result<Vec<String>, ServiceError> {
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

        let mut added_tags = Vec::new();

        for tag_name in &self.tags {
            // Check if the tag already exists, if not create it
            let tag = tags::table
                .filter(tags::name.eq(tag_name))
                .first::<Tag>(conn)
                .or_else(|_| {
                    diesel::insert_into(tags::table)
                        .values(Tag {
                            id: Uuid::new_v4(),
                            name: tag_name.clone(),
                        })
                        .get_result::<Tag>(conn)
                })?;

            // Check if the ticket already has this tag
            let existing_ticket_tag = ticket_tags::table
                .filter(ticket_tags::ticket_id.eq(self.ticket_id))
                .filter(ticket_tags::tag_id.eq(tag.id))
                .first::<TicketTag>(conn);

            if existing_ticket_tag.is_err() {
                // If the ticket doesn't have this tag, add it
                diesel::insert_into(ticket_tags::table)
                    .values(TicketTag {
                        id: Uuid::new_v4(),
                        ticket_id: self.ticket_id,
                        tag_id: tag.id,
                    })
                    .execute(conn)?;

                added_tags.push(tag_name.clone());
            }
        }

        Ok(added_tags)
    }

    async fn log_and_trigger_event(&self, event_sender: Arc<EventSender>, added_tags: &[String]) -> Result<(), ServiceError> {
        info!("Tags added to ticket ID {}: {:?}", self.ticket_id, added_tags);
        event_sender.send(Event::TagsAddedToTicket(self.ticket_id, added_tags.to_vec(), self.added_by))
            .await
            .map_err(|e| {
                error!("Failed to send event for added tags: {}", e);
                ServiceError::EventError(e.to_string())
            })
    }
}