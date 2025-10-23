use crate::{
    commands::Command,
    db::DbPool,
    errors::ServiceError,
    events::{Event, EventSender},
    models::{
        r#return::ReturnStatus,
        return_entity::{self},
    },
};
use sea_orm::{Set, *};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info, instrument};
use uuid::Uuid;
use validator::Validate;

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct ApproveReturnCommand {
    pub return_id: Uuid,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ApproveReturnResult {
    pub id: Uuid,
    pub object: String,
    pub approved: bool,
}

#[async_trait::async_trait]
impl Command for ApproveReturnCommand {
    type Result = ApproveReturnResult;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(
        &self,
        db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        let db = db_pool.as_ref();

        let approved_return = self.approve_return(db).await?;

        self.log_and_trigger_event(&event_sender, &approved_return)
            .await?;

        Ok(ApproveReturnResult {
            id: approved_return.id,
            object: "return".to_string(),
            approved: true,
        })
    }
}

impl ApproveReturnCommand {
    async fn approve_return(
        &self,
        db: &DatabaseConnection,
    ) -> Result<return_entity::Model, ServiceError> {
        let return_request = return_entity::Entity::find_by_id(self.return_id)
            .one(db)
            .await
            .map_err(|e| ServiceError::db_error(e))?
            .ok_or_else(|| {
                let msg = format!("Return request {} not found", self.return_id);
                error!("{}", msg);
                ServiceError::NotFound(msg)
            })?;

        let mut return_request: return_entity::ActiveModel = return_request.into();
        return_request.status = Set(ReturnStatus::Approved.as_str().to_string());

        let updated_return = return_request.update(db).await.map_err(|e| {
            let msg = format!("Failed to update return request {}: {}", self.return_id, e);
            error!("{}", msg);
            ServiceError::db_error(e)
        })?;

        Ok(updated_return)
    }

    async fn log_and_trigger_event(
        &self,
        event_sender: &EventSender,
        _approved_return: &return_entity::Model,
    ) -> Result<(), ServiceError> {
        info!("Return request approved for return ID: {}", self.return_id);
        event_sender
            .send(Event::ReturnApproved(self.return_id))
            .await
            .map_err(|e| {
                let msg = format!("Failed to send event for approved return: {}", e);
                error!("{}", msg);
                ServiceError::EventError(msg)
            })
    }
}
