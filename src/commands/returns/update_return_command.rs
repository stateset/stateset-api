use crate::{
    commands::Command,
    db::DbPool,
    errors::ServiceError,
    events::{Event, EventSender},
    models::return_entity::{self, Entity as Return},
};
use sea_orm::{Set, *};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info, instrument};
use uuid::Uuid;
use validator::Validate;

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct UpdateReturnCommand {
    pub return_id: Uuid,
    #[validate(length(min = 1))]
    pub reason: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateReturnResult {
    pub id: Uuid,
    pub reason: String,
    pub updated_at: chrono::NaiveDateTime,
}

#[async_trait::async_trait]
impl Command for UpdateReturnCommand {
    type Result = UpdateReturnResult;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(
        &self,
        db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        self.validate()
            .map_err(|e| ServiceError::ValidationError(e.to_string()))?;
        let db = db_pool.as_ref();
        let updated = self.update_return(db).await?;

        event_sender
            .send(Event::with_data(format!(
                "return_updated:{}",
                self.return_id
            )))
            .await
            .map_err(ServiceError::EventError)?;

        Ok(UpdateReturnResult {
            id: updated.id,
            reason: updated.reason,
            updated_at: updated.updated_at,
        })
    }
}

impl UpdateReturnCommand {
    async fn update_return(
        &self,
        db: &DatabaseConnection,
    ) -> Result<return_entity::Model, ServiceError> {
        let return_request = Return::find_by_id(self.return_id)
            .one(db)
            .await
            .map_err(|e| {
                let msg = format!("Failed to fetch return request: {}", e);
                error!("{}", msg);
                ServiceError::db_error(msg)
            })?
            .ok_or_else(|| {
                let msg = format!("Return {} not found", self.return_id);
                error!("{}", msg);
                ServiceError::NotFound(msg)
            })?;

        let mut active: return_entity::ActiveModel = return_request.into();

        if let Some(reason) = &self.reason {
            active.reason = Set(reason.clone());
        }

        active.update(db).await.map_err(|e| {
            let msg = format!("Failed to update return {}: {}", self.return_id, e);
            error!("{}", msg);
            ServiceError::db_error(e)
        })
    }
}
