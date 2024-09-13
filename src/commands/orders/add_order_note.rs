use std::sync::Arc;
use sea_orm::*;
use crate::{
    db::DbPool,
    errors::ServiceError,
    events::{Event, EventSender},
    models::{
        order_note_entity::{self, Entity as OrderNote},
    },
};
use serde::{Deserialize, Serialize};
use tracing::{error, info, instrument};
use uuid::Uuid;
use validator::Validate;
use prometheus::IntCounter;
use chrono::{DateTime, Utc};
use lazy_static::lazy_static;

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
    pub order_id: Uuid,
    #[validate(length(min = 1, max = 1000))]
    pub note: String,
    pub is_customer_visible: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AddOrderNoteResult {
    pub id: Uuid,
    pub order_id: Uuid,
    pub note: String,
    pub is_customer_visible: bool,
    pub created_at: DateTime<Utc>,
}

#[async_trait::async_trait]
impl Command for AddOrderNoteCommand {
    type Result = AddOrderNoteResult;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(
        &self,
        db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        self.validate().map_err(|e| {
            ORDER_NOTE_ADD_FAILURES.inc();
            let msg = format!("Invalid input: {}", e);
            error!("{}", msg);
            ServiceError::ValidationError(msg)
        })?;

        let db = db_pool.as_ref();

        let saved_note = self.add_note_to_order(db).await?;

        self.log_and_trigger_event(&event_sender, &saved_note).await?;

        ORDER_NOTES_ADDED.inc();

        Ok(AddOrderNoteResult {
            id: saved_note.id,
            order_id: saved_note.order_id,
            note: saved_note.note,
            is_customer_visible: saved_note.is_customer_visible,
            created_at: saved_note.created_at.and_utc(),
        })
    }
}

impl AddOrderNoteCommand {
    async fn add_note_to_order(
        &self,
        db: &DatabaseConnection,
    ) -> Result<order_note_entity::Model, ServiceError> {
        let new_note = order_note_entity::ActiveModel {
            order_id: Set(self.order_id),
            note: Set(self.note.clone()),
            is_customer_visible: Set(self.is_customer_visible),
            created_at: Set(Utc::now().naive_utc()),
            ..Default::default()
        };

        new_note.insert(db).await.map_err(|e| {
            ORDER_NOTE_ADD_FAILURES.inc();
            let msg = format!("Failed to add note to order {}: {}", self.order_id, e);
            error!("{}", msg);
            ServiceError::DatabaseError(msg)
        })
    }

    async fn log_and_trigger_event(
        &self,
        event_sender: &EventSender,
        saved_note: &order_note_entity::Model,
    ) -> Result<(), ServiceError> {
        info!(
            order_id = %self.order_id,
            is_customer_visible = %self.is_customer_visible,
            "Note added to order successfully"
        );

        event_sender
            .send(Event::OrderNoteAdded(self.order_id, saved_note.id))
            .await
            .map_err(|e| {
                ORDER_NOTE_ADD_FAILURES.inc();
                let msg = format!("Failed to send event for added order note: {}", e);
                error!("{}", msg);
                ServiceError::EventError(msg)
            })
    }
}