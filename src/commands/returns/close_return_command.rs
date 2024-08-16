use std::sync::Arc;
use sea_orm::*;
use crate::{errors::ServiceError, db::DbPool, models::{return_entity, return_entity::Entity as Return}};
use crate::models::return_entity::ReturnStatus;
use crate::events::{Event, EventSender};
use tracing::{info, error, instrument};
use serde::{Serialize, Deserialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct CloseReturnCommand {
    pub return_id: i32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CloseReturnResult {
    pub id: String,
    pub object: String,
    pub completed: bool,
}

#[async_trait::async_trait]
impl Command for CloseReturnCommand {
    type Result = CloseReturnResult;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        let db = db_pool.get().map_err(|e| {
            error!("Failed to get database connection: {}", e);
            ServiceError::DatabaseError("Failed to get database connection".into())
        })?;

        let completed_return = self.close_return(&db).await?;

        self.log_and_trigger_event(event_sender, &completed_return).await?;

        Ok(CloseReturnResult {
            id: completed_return.id.to_string(),
            object: "return".to_string(),
            completed: true,
        })
    }
}

impl CloseReturnCommand {
    async fn close_return(&self, db: &DatabaseConnection) -> Result<return_entity::Model, ServiceError> {
        let return_request = Return::find_by_id(self.return_id)
            .one(db)
            .await
            .map_err(|e| {
                error!("Database error: {}", e);
                ServiceError::DatabaseError(format!("Database error: {}", e))
            })?
            .ok_or_else(|| {
                error!("Return request not found: {}", self.return_id);
                ServiceError::NotFound(format!("Return request with ID {} not found", self.return_id))
            })?;

        let mut return_request: return_entity::ActiveModel = return_request.into();
        return_request.status = Set(ReturnStatus::Closed.to_string());

        return_request
            .update(db)
            .await
            .map_err(|e| {
                error!("Failed to close return request: {}", e);
                ServiceError::DatabaseError(format!("Failed to close return request: {}", e))
            })
    }

    async fn log_and_trigger_event(&self, event_sender: Arc<EventSender>, completed_return: &return_entity::Model) -> Result<(), ServiceError> {
        info!("Return request closed for return ID: {}", self.return_id);
        event_sender.send(Event::ReturnClosed(self.return_id))
            .await
            .map_err(|e| {
                error!("Failed to send ReturnClosed event for return ID {}: {}", self.return_id, e);
                ServiceError::EventError(e.to_string())
            })
    }
}