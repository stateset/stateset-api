use crate::{
    commands::Command,
    db::DbPool,
    errors::ServiceError,
    events::{Event, EventSender},
    models::return_entity::{self, Entity as Return},
};
use sea_orm::*;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info, instrument};
use uuid::Uuid;
use validator::Validate;

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct DeleteReturnCommand {
    pub return_id: Uuid,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeleteReturnResult {
    pub id: Uuid,
    pub object: String,
    pub deleted: bool,
}

#[async_trait::async_trait]
impl Command for DeleteReturnCommand {
    type Result = DeleteReturnResult;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(
        &self,
        db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        let db = db_pool.as_ref();

        let deleted_return = self.delete_return(db).await?;

        self.log_and_trigger_event(&event_sender, &deleted_return)
            .await?;

        Ok(DeleteReturnResult {
            id: Uuid::parse_str(&deleted_return.id).unwrap_or_else(|_| Uuid::new_v4()),
            deleted: true,
        })
    }
}

impl DeleteReturnCommand {
    async fn delete_return(
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
                let msg = format!("Return {} not found", self.return_id);
                error!("{}", msg);
                ServiceError::NotFound(msg)
            })?;

        // Store the return details before deletion
        let deleted_return = return_request.clone();

        // Delete the return request
        Return::delete_by_id(self.return_id)
            .exec(db)
            .await
            .map_err(|e| {
                let msg = format!("Failed to delete return {}: {}", self.return_id, e);
                error!("{}", msg);
                ServiceError::DatabaseError(e)
            })?;

        Ok(deleted_return)
    }

    async fn log_and_trigger_event(
        &self,
        event_sender: &EventSender,
        deleted_return: &return_entity::Model,
    ) -> Result<(), ServiceError> {
        info!("Return request deleted for return ID: {}", self.return_id);
        event_sender
            .send(Event::ReturnUpdated(self.return_id)) // Using Updated as deletion is a form of update
            .await
            .map_err(|e| {
                let msg = format!("Failed to send event for deleted return: {}", e);
                error!("{}", msg);
                ServiceError::EventError(msg)
            })
    }
}
