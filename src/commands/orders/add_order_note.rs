use async_trait::async_trait;;
use serde::{Serialize, Deserialize};
use std::sync::Arc;
use sea_orm::*;
use crate::{errors::ServiceError, db::DbPool, models::{order_note_entity, order_note_entity::Entity as OrderNote}};
use crate::events::{Event, EventSender};
use validator::Validate;
use tracing::{info, error, instrument};
use prometheus::IntCounter;
use chrono::Utc;
use lazy_static::lazy_static

lazy_static! {
    static ref ORDER_NOTES_ADDED: IntCounter = 
        IntCounter::new("order_notes_added_total", "Total number of notes added to orders")
            .expect("metric can be created");

    static ref ORDER_NOTE_ADD_FAILURES: IntCounter = 
        IntCounter::new("order_note_add_failures_total", "Total number of failed note additions to orders")
            .expect("metric can be created");
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct AddOrderNoteCommand {
    pub order_id: i32,
    #[validate(length(min = 1, max = 1000))]
    pub note: String,
    pub is_customer_visible: bool,
}

#[async_trait]
impl Command for AddOrderNoteCommand {
    type Result = order_note_entity::Model;

    #[instrument(skip(db_pool, event_sender))]
    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        // Validate the command
        if let Err(e) = self.validate() {
            ORDER_NOTE_ADD_FAILURES.inc();
            error!("Invalid AddOrderNoteCommand: {}", e);
            return Err(ServiceError::ValidationError(e.to_string()));
        }

        let db = db_pool.get().map_err(|e| {
            ORDER_NOTE_ADD_FAILURES.inc();
            error!("Failed to get database connection: {}", e);
            ServiceError::DatabaseError
        })?;

        // Create a new OrderNote to be added to the order
        let new_note = order_note_entity::ActiveModel {
            order_id: Set(self.order_id),
            note: Set(self.note.clone()),
            is_customer_visible: Set(self.is_customer_visible),
            created_at: Set(Utc::now().naive_utc()),
            ..Default::default() // This will set default values for other fields
        };

        // Insert the new note into the order_notes table
        let saved_note = new_note.insert(&db).await.map_err(|e| {
            ORDER_NOTE_ADD_FAILURES.inc();
            error!("Failed to add note to order {}: {}", self.order_id, e);
            ServiceError::DatabaseError
        })?;

        // Trigger an event indicating that a note was added to the order
        if let Err(e) = event_sender.send(Event::OrderNoteAdded(self.order_id, saved_note.id)).await {
            ORDER_NOTE_ADD_FAILURES.inc();
            error!("Failed to send OrderNoteAdded event for order {}: {}", self.order_id, e);
            return Err(ServiceError::EventError(e.to_string()));
        }

        ORDER_NOTES_ADDED.inc();

        info!(
            order_id = %self.order_id,
            is_customer_visible = %self.is_customer_visible,
            "Note added to order successfully"
        );

        Ok(saved_note)
    }
}