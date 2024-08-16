use std::sync::Arc;
use sea_orm::*;
use crate::{errors::ServiceError, db::DbPool, models::{return_entity, return_entity::Entity as Return}};
use crate::events::{Event, EventSender};
use validator::Validate;
use tracing::{info, error, instrument};
use serde::{Serialize, Deserialize};
use async_trait::async_trait;;

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct InitiateReturnCommand {
    pub order_id: i32,
    
    #[validate(length(min = 1))]
    pub reason: String,
}

#[async_trait]
impl Command for InitiateReturnCommand {
    type Result = return_entity::Model;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        let db = db_pool.get().map_err(|e| {
            error!("Failed to get database connection: {}", e);
            ServiceError::DatabaseError("Failed to get database connection".into())
        })?;

        let saved_return = self.create_return_request(&db).await?;

        self.log_and_trigger_event(event_sender, &saved_return).await?;

        Ok(saved_return)
    }
}

impl InitiateReturnCommand {
    async fn create_return_request(&self, db: &DatabaseConnection) -> Result<return_entity::Model, ServiceError> {
        let return_request = return_entity::ActiveModel {
            order_id: Set(self.order_id),
            reason: Set(self.reason.clone()),
            status: Set(ReturnStatus::Pending.to_string()),
            // Add other fields as necessary, like created_at, updated_at, etc.
            ..Default::default() // This will set default values for other fields
        };

        return_request
            .insert(db)
            .await
            .map_err(|e| {
                error!("Failed to initiate return for order ID {}: {}", self.order_id, e);
                ServiceError::DatabaseError(format!("Failed to initiate return: {}", e))
            })
    }

    async fn log_and_trigger_event(&self, event_sender: Arc<EventSender>, saved_return: &return_entity::Model) -> Result<(), ServiceError> {
        info!("Return initiated for order ID: {}. Reason: {}", self.order_id, self.reason);
        event_sender.send(Event::ReturnInitiated(saved_return.id))
            .await
            .map_err(|e| {
                error!("Failed to send ReturnInitiated event for return ID {}: {}", saved_return.id, e);
                ServiceError::EventError(e.to_string())
            })
    }
}