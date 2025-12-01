use crate::{
    commands::Command,
    db::DbPool,
    errors::ServiceError,
    events::{Event, EventSender},
    models::{
        order_entity::{self, Entity as Order},
        order_note_entity::{self as note_entity, Entity as OrderNote},
    },
};
use chrono::{DateTime, Utc};
use lazy_static::lazy_static;
use prometheus::IntCounter;
use sea_orm::{*, Set};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info, instrument};
use uuid::Uuid;
use validator::Validate;

lazy_static! {
    static ref ORDER_NOTES_ADDED: IntCounter = IntCounter::new(
        "order_notes_added_total",
        "Total number of notes added to orders"
    )
    .expect("metric can be created");
    static ref ORDER_NOTE_ADD_FAILURES: IntCounter = IntCounter::new(
        "order_note_add_failures_total",
        "Total number of failed note additions to orders"
    )
    .expect("metric can be created");
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct AddOrderNoteCommand {
    pub order_id: Uuid,
    #[validate(length(min = 1, max = 1000, message = "Note must be between 1 and 1000 characters"))]
    pub note: String,
    #[validate(length(min = 1, max = 100, message = "Created by must be between 1 and 100 characters"))]
    pub created_by: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AddOrderNoteResult {
    pub note_id: Uuid,
    pub order_id: Uuid,
    pub note: String,
    pub created_by: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
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

        let order = Order::find_by_id(self.order_id)
            .one(db)
            .await
            .map_err(|e| ServiceError::db_error(e))?
            .ok_or_else(|| {
                let msg = format!("Order {} not found", self.order_id);
                error!("{}", msg);
                ServiceError::NotFound(msg)
            })?;

        let saved_note = self.create_note(db).await?;

        self.log_and_trigger_event(&event_sender, &saved_note)
            .await?;

        ORDER_NOTES_ADDED.inc();

        Ok(AddOrderNoteResult {
            note_id: saved_note.id,
            order_id: saved_note.order_id,
            note: saved_note.note,
            created_by: saved_note.created_by,
            created_at: saved_note.created_at,
        })
    }
}

impl AddOrderNoteCommand {
    async fn create_note(&self, db: &DatabaseConnection) -> Result<note_entity::Model, ServiceError> {
        let new_note = note_entity::ActiveModel {
            order_id: Set(self.order_id),
            note: Set(self.note.clone()),
            created_by: Set(Some(self.created_by.clone())),
            ..Default::default()
        };

        new_note.insert(db).await.map_err(|e| {
            let msg = format!("Failed to create note for order {}: {}", self.order_id, e);
            error!("{}", msg);
            ServiceError::db_error(e)
        })
    }

    async fn log_and_trigger_event(
        &self,
        event_sender: &EventSender,
        saved_note: &note_entity::Model,
    ) -> Result<(), ServiceError> {
        info!(
            order_id = %self.order_id,
            note_id = %saved_note.id,
            created_by = %self.created_by,
            "Order note added successfully"
        );

        event_sender
            .send(Event::OrderUpdated {
                order_id: self.order_id,
                checkout_session_id: None,
                status: None,
                refunds: vec![],
            })
            .await
            .map_err(|e| {
                ORDER_NOTE_ADD_FAILURES.inc();
                let msg = format!("Failed to send order note added event: {}", e);
                error!("{}", msg);
                ServiceError::EventError(msg)
            })?;
    }
}
