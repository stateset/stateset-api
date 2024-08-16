use std::sync::Arc;
use sea_orm::*;
use crate::{errors::ServiceError, db::DbPool, models::{return_entity, return_entity::Entity as Return}};
use crate::models::return_entity::ReturnStatus;
use crate::events::{Event, EventSender};
use tracing::{info, error, instrument};
use serde::{Serialize, Deserialize};
use async_trait::async_trait;;

#[derive(Debug, Serialize, Deserialize)]
pub struct ReopenReturnCommand {
    pub return_id: i32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ReopenReturnResult {
    pub id: String,
    pub object: String,
    pub reopened: bool,
}

#[async_trait]
impl Command for ReopenReturnCommand {
    type Result = ReopenReturnResult;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        let db = db_pool.get().map_err(|e| {
            error!("Failed to get database connection: {}", e);
            ServiceError::DatabaseError("Failed to get database connection".into())
        })?;

        let reopened_return = self.reopen_return(&db).await?;

        self.log_and_trigger_event(event_sender, &reopened_return).await?;

        Ok(ReopenReturnResult {
            id: reopened_return.id.to_string(),
            object: "return".to_string(),
            reopened: true,
        })
    }
}

impl ReopenReturnCommand {
    async fn reopen_return(&self, db: &DatabaseConnection) -> Result<return_entity::Model, ServiceError> {
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
        return_request.status = Set(ReturnStatus::Open.to_string());

        return_request
            .update(db)
            .await
            .map_err(|e| {
                error!("Failed to reopen return request: {}", e);
                ServiceError::DatabaseError(format!("Failed to reopen return request: {}", e))
            })
    }

    async fn log_and_trigger_event(&self, event_sender: Arc<EventSender>, reopened_return: &return_entity::Model) -> Result<(), ServiceError> {
        info!("Return request reopened for return ID: {}", self.return_id);
        event_sender.send(Event::ReturnReopened(self.return_id))
            .await
            .map_err(|e| {
                error!("Failed to send ReturnReopened event for return ID {}: {}", self.return_id, e);
                ServiceError::EventError(e.to_string())
            })
    }
}