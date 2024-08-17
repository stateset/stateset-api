use std::sync::Arc;
use sea_orm::*;
use crate::{errors::ServiceError, db::DbPool, models::{return_entity, return_entity::Entity as Return}};
use crate::events::{Event, EventSender};
use validator::Validate;
use tracing::{info, error, instrument};
use serde::{Serialize, Deserialize};

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct DeleteReturnCommand {
    pub return_id: i32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeleteReturnResult {
    pub id: String,
    pub object: String,
    pub deleted: bool,
}

#[async_trait::async_trait]
impl Command for DeleteReturnCommand {
    type Result = DeleteReturnResult;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        let db = db_pool.get().map_err(|e| {
            error!("Database connection error: {}", e);
            ServiceError::DatabaseError("Failed to get database connection".into())
        })?;

        let deleted_return = self.delete_return(&db).await?;

        self.log_and_trigger_event(event_sender, &deleted_return).await?;

        Ok(DeleteReturnResult {
            id: deleted_return.id.to_string(),
            object: "return".to_string(),
            deleted: true,
        })
    }
}

impl DeleteReturnCommand {
    async fn delete_return(&self, db: &DatabaseConnection) -> Result<return_entity::Model, ServiceError> {
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

        // Store the return details before deletion
        let deleted_return = return_request.clone();

        // Delete the return request
        Return::delete_by_id(self.return_id)
            .exec(db)
            .await
            .map_err(|e| {
                error!("Failed to delete return request: {}", e);
                ServiceError::DatabaseError(format!("Database error: {}", e))
            })?;

        Ok(deleted_return)
    }

    async fn log_and_trigger_event(&self, event_sender: Arc<EventSender>, deleted_return: &return_entity::Model) -> Result<(), ServiceError> {
        info!("Return request deleted for return ID: {}", self.return_id);
        event_sender.send(Event::ReturnDeleted(self.return_id))
            .await
            .map_err(|e| {
                error!("Failed to send event for deleted return: {}", e);
                ServiceError::EventError(e.to_string())
            })
    }
}