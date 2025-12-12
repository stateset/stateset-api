use crate::{
    commands::Command,
    db::DbPool,
    errors::ServiceError,
    events::{Event, EventSender},
    models::{
        r#return::ReturnStatus,
        return_entity::{self, Entity as Return},
    },
};
use sea_orm::{Set, *};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info, instrument};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
pub struct ReopenReturnCommand {
    pub return_id: Uuid,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ReopenReturnResult {
    pub id: Uuid,
    pub object: String,
    pub reopened: bool,
    pub status: String,
}

#[async_trait::async_trait]
impl Command for ReopenReturnCommand {
    type Result = ReopenReturnResult;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(
        &self,
        db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        let db = db_pool.as_ref();

        let reopened_return = self.reopen_return(db).await?;

        self.log_and_trigger_event(&event_sender, &reopened_return)
            .await?;

        Ok(ReopenReturnResult {
            id: reopened_return.id,
            object: "return".to_string(),
            reopened: true,
            status: reopened_return.status,
        })
    }
}

impl ReopenReturnCommand {
    async fn reopen_return(
        &self,
        db: &DatabaseConnection,
    ) -> Result<return_entity::Model, ServiceError> {
        let return_request = Return::find_by_id(self.return_id)
            .one(db)
            .await
            .map_err(|e| {
                let msg = format!("Failed to find return request: {}", e);
                error!("{}", msg);
                ServiceError::db_error(msg)
            })?
            .ok_or_else(|| {
                let msg = format!("Return {} not found", self.return_id);
                error!("{}", msg);
                ServiceError::NotFound(msg)
            })?;

        let mut return_request: return_entity::ActiveModel = return_request.into();
        return_request.status = Set(ReturnStatus::Requested.as_str().to_owned()); // Use Requested instead of Open

        let updated_return = return_request.update(db).await.map_err(|e| {
            let msg = format!("Failed to update return {}: {}", self.return_id, e);
            error!("{}", msg);
            ServiceError::db_error(e)
        })?;

        Ok(updated_return)
    }

    async fn log_and_trigger_event(
        &self,
        event_sender: &EventSender,
        _reopened_return: &return_entity::Model,
    ) -> Result<(), ServiceError> {
        info!("Return request reopened for return ID: {}", self.return_id);
        event_sender
            .send(Event::ReturnUpdated(self.return_id))
            .await
            .map_err(|e| {
                let msg = format!("Failed to send event for reopened return: {}", e);
                error!("{}", msg);
                ServiceError::EventError(msg)
            })
    }
}
