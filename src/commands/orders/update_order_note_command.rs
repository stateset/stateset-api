use crate::{
    commands::Command,
    db::DbPool,
    errors::ServiceError,
    events::{Event, EventSender},
    models::order_note_entity::{self, Entity as OrderNote},
};
use chrono::Utc;
use sea_orm::{*, Set};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info, instrument};
use uuid::Uuid;
use validator::Validate;

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct UpdateOrderNoteCommand {
    pub order_id: Uuid,
    pub note_id: i32,
    #[validate(length(min = 1, max = 1000))]
    pub new_note: String,
}

#[async_trait::async_trait]
impl Command for UpdateOrderNoteCommand {
    type Result = order_note_entity::Model;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(
        &self,
        db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        let db = db_pool.as_ref();

        let updated_note = self.update_note(db).await?;

        self.log_and_trigger_event(&event_sender, &updated_note)
            .await?;

        Ok(updated_note)
    }
}

impl UpdateOrderNoteCommand {
    async fn update_note(
        &self,
        db: &DatabaseConnection,
    ) -> Result<order_note_entity::Model, ServiceError> {
        let note = OrderNote::find_by_id(self.note_id)
            .one(db)
            .await
            .map_err(|e| {
                error!("Failed to find note: {}", e);
                ServiceError::db_error(e)
            })?
            .ok_or_else(|| {
                let msg = format!("Note {} not found", self.note_id);
                error!("{}", msg);
                ServiceError::NotFound(msg)
            })?;

        let mut active: order_note_entity::ActiveModel = note.into();
        active.note = Set(self.new_note.clone());
        // Note: order_note_entity doesn't have updated_at field, only created_at
        // active.updated_at = Set(Some(Utc::now()));

        active.update(db).await.map_err(|e| {
            let msg = format!("Failed to update order note: {}", e);
            error!("{}", msg);
            ServiceError::db_error(e)
        })
    }

    async fn log_and_trigger_event(
        &self,
        event_sender: &EventSender,
        updated_note: &order_note_entity::Model,
    ) -> Result<(), ServiceError> {
        info!(order_id = %self.order_id, note_id = %self.note_id, "Order note updated");
        event_sender
            .send(Event::OrderUpdated {
                order_id: self.order_id,
                checkout_session_id: None,
                status: None,
                refunds: vec![],
            })
            .await
            .map_err(|e| {
                let msg = format!("Failed to send event for updated order note: {}", e);
                error!("{}", msg);
                ServiceError::EventError(msg)
            })?;
        Ok(())
    }
}
