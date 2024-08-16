use std::sync::Arc;
use sea_orm::*;
use crate::{errors::ServiceError, db::DbPool, models::{return_entity, return_entity::Entity as Return}};
use crate::models::return_entity::ReturnStatus;
use crate::events::{Event, EventSender};
use validator::Validate;
use tracing::{info, error, instrument};
use async_trait::async_trait;;
use serde::{Serialize, Deserialize};

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct RejectReturnCommand {
    pub return_id: i32,

    #[validate(length(min = 1))]
    pub reason: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RejectReturnResult {
    pub id: String,
    pub object: String,
    pub rejected: bool,
    pub reason: String,
}

#[async_trait]
impl Command for RejectReturnCommand {
    type Result = RejectReturnResult;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        let db = db_pool.get().map_err(|e| {
            error!("Failed to get database connection: {}", e);
            ServiceError::DatabaseError("Failed to get database connection".into())
        })?;

        let rejected_return = self.reject_return(&db).await?;

        self.log_and_trigger_event(event_sender, &rejected_return).await?;

        Ok(RejectReturnResult {
            id: rejected_return.id.to_string(),
            object: "return".to_string(),
            rejected: true,
            reason: self.reason.clone(),
        })
    }
}

impl RejectReturnCommand {
    async fn reject_return(&self, db: &DatabaseConnection) -> Result<return_entity::Model, ServiceError> {
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
        return_request.status = Set(ReturnStatus::Rejected.to_string());
        return_request.reason = Set(Some(self.reason.clone()));

        return_request
            .update(db)
            .await
            .map_err(|e| {
                error!("Failed to reject return request: {}", e);
                ServiceError::DatabaseError(format!("Failed to reject return request: {}", e))
            })
    }

    async fn log_and_trigger_event(&self, event_sender: Arc<EventSender>, rejected_return: &return_entity::Model) -> Result<(), ServiceError> {
        info!("Return request rejected for return ID: {}. Reason: {}", self.return_id, self.reason);
        event_sender.send(Event::ReturnRejected(self.return_id))
            .await
            .map_err(|e| {
                error!("Failed to send ReturnRejected event for return ID {}: {}", self.return_id, e);
                ServiceError::EventError(e.to_string())
            })
    }
}