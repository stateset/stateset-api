use std::sync::Arc;
use sea_orm::*;
use crate::{
    db::DbPool,
    errors::ServiceError,
    events::{Event, EventSender},
    models::{
        return_entity::{self, Entity as Return},
        return_entity::ReturnStatus,
    },
};
use serde::{Deserialize, Serialize};
use tracing::{error, info, instrument};
use uuid::Uuid;
use validator::Validate;

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct RejectReturnCommand {
    pub return_id: Uuid,

    #[validate(length(min = 1, message = "Reason cannot be empty"))]
    pub reason: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RejectReturnResult {
    pub id: Uuid,
    pub object: String,
    pub rejected: bool,
    pub reason: String,
}

#[async_trait::async_trait]
impl Command for RejectReturnCommand {
    type Result = RejectReturnResult;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(
        &self,
        db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        self.validate().map_err(|e| {
            let msg = format!("Invalid input: {}", e);
            error!("{}", msg);
            ServiceError::ValidationError(msg)
        })?;

        let db = db_pool.as_ref();

        let rejected_return = self.reject_return(db).await?;

        self.log_and_trigger_event(&event_sender, &rejected_return)
            .await?;

        Ok(RejectReturnResult {
            id: rejected_return.id,
            object: "return".to_string(),
            rejected: true,
            reason: self.reason.clone(),
        })
    }
}

impl RejectReturnCommand {
    async fn reject_return(
        &self,
        db: &DatabaseConnection,
    ) -> Result<return_entity::Model, ServiceError> {
        let return_request = Return::find_by_id(self.return_id)
            .one(db)
            .await
            .map_err(|e| {
                let msg = format!("Failed to find return request: {}", e);
                error!("{}", msg);
                ServiceError::DatabaseError(msg)
            })?
            .ok_or_else(|| {
                let msg = format!("Return request with ID {} not found", self.return_id);
                error!("{}", msg);
                ServiceError::NotFound(msg)
            })?;

        let mut return_request: return_entity::ActiveModel = return_request.into();
        return_request.status = Set(ReturnStatus::Rejected.to_string());
        return_request.reason = Set(Some(self.reason.clone()));

        let updated_return = return_request
            .update(db)
            .await
            .map_err(|e| {
                let msg = format!("Failed to reject return request: {}", e);
                error!("{}", msg);
                ServiceError::DatabaseError(msg)
            })?;

        Ok(updated_return)
    }

    async fn log_and_trigger_event(
        &self,
        event_sender: &EventSender,
        rejected_return: &return_entity::Model,
    ) -> Result<(), ServiceError> {
        info!("Return request rejected for return ID: {}. Reason: {}", self.return_id, self.reason);
        event_sender
            .send(Event::ReturnRejected(self.return_id))
            .await
            .map_err(|e| {
                let msg = format!("Failed to send event for rejected return: {}", e);
                error!("{}", msg);
                ServiceError::EventError(msg)
            })
    }
}