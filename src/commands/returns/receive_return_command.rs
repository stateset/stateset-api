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
            .send(Event::ReturnUpdated(self.return_id))
            .await
            .map_err(|e| ServiceError::EventError(e.to_string()))?;

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
                ServiceError::db_error(e)
            })?
            .ok_or_else(|| {
                let msg = format!("Return {} not found", self.return_id);
                error!("{}", msg);
                ServiceError::NotFound(msg)
            })?;

        let mut active: return_entity::ActiveModel = return_request.into();
        active.status = Set(ReturnStatus::Received.as_str().to_owned());

        let updated = active.update(db).await.map_err(|e| {
            let msg = format!("Failed to update return {}: {}", self.return_id, e);
            error!("{}", msg);
            ServiceError::db_error(e)
        })?;

        info!("Return marked as received: {}", self.return_id);
        Ok(updated)
    }
}
