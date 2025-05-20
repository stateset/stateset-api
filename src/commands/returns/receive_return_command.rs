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
    commands::Command,
};
use serde::{Deserialize, Serialize};
use tracing::{error, info, instrument};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
pub struct ReceiveReturnCommand {
    pub return_id: Uuid,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ReceiveReturnResult {
    pub id: Uuid,
    pub status: String,
}

#[async_trait::async_trait]
impl Command for ReceiveReturnCommand {
    type Result = ReceiveReturnResult;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(
        &self,
        db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        let db = db_pool.as_ref();

        let received_return = self.mark_received(db).await?;

        event_sender
            .send(Event::ReturnProcessed(self.return_id))
            .await
            .map_err(ServiceError::EventError)?;

        Ok(ReceiveReturnResult {
            id: received_return.id,
            status: received_return.status,
        })
    }
}

impl ReceiveReturnCommand {
    async fn mark_received(
        &self,
        db: &DatabaseConnection,
    ) -> Result<return_entity::Model, ServiceError> {
        let return_request = Return::find_by_id(self.return_id)
            .one(db)
            .await
            .map_err(|e| {
                let msg = format!("Failed to fetch return request: {}", e);
                error!("{}", msg);
                ServiceError::DatabaseError(msg)
            })?
            .ok_or_else(|| {
                let msg = format!("Return request with ID {} not found", self.return_id);
                error!("{}", msg);
                ServiceError::NotFound(msg)
            })?;

        let mut active: return_entity::ActiveModel = return_request.into();
        active.status = Set(ReturnStatus::Received.to_string());

        let updated = active.update(db).await.map_err(|e| {
            let msg = format!("Failed to update return status: {}", e);
            error!("{}", msg);
            ServiceError::DatabaseError(msg)
        })?;

        info!("Return marked as received", return_id = %self.return_id);
        Ok(updated)
    }
}
