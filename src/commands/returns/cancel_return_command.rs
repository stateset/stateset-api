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
pub struct CancelReturnCommand {
    pub return_id: Uuid,
    
    #[validate(length(min = 1, message = "Reason cannot be empty"))]
    pub reason: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CancelReturnResult {
    pub id: Uuid,
    pub object: String,
    pub cancelled: bool,
    pub reason: String,
}

#[async_trait::async_trait]
impl Command for CancelReturnCommand {
    type Result = CancelReturnResult;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(
        &self,
        db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        let db = db_pool.as_ref();

        let cancelled_return = self.cancel_return(db).await?;

        self.log_and_trigger_event(&event_sender, &cancelled_return)
            .await?;

        Ok(CancelReturnResult {
            id: cancelled_return.id,
            object: "return".to_string(),
            cancelled: true,
            reason: self.reason.clone(),
        })
    }
}

impl CancelReturnCommand {
    async fn cancel_return(
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
        return_request.status = Set(ReturnStatus::Cancelled.to_string());
        return_request.reason = Set(Some(self.reason.clone()));

        let updated_return = return_request.update(db).await.map_err(|e| {
            let msg = format!("Failed to update return request: {}", e);
            error!("{}", msg);
            ServiceError::DatabaseError(msg)
        })?;

        Ok(updated_return)
    }

    async fn log_and_trigger_event(
        &self,
        event_sender: &EventSender,
        cancelled_return: &return_entity::Model,
    ) -> Result<(), ServiceError> {
        info!("Return request cancelled for return ID: {}. Reason: {}", self.return_id, self.reason);
        event_sender
            .send(Event::ReturnCancelled(self.return_id))
            .await
            .map_err(|e| {
                let msg = format!("Failed to send event for cancelled return: {}", e);
                error!("{}", msg);
                ServiceError::EventError(msg)
            })
    }
}