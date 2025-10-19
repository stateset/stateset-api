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
use validator::Validate;

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct RefundReturnCommand {
    pub return_id: Uuid,
    #[validate(range(min = 0.0))]
    pub refund_amount: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RefundReturnResult {
    pub id: Uuid,
    pub status: String,
    pub refund_amount: f64,
}

#[async_trait::async_trait]
impl Command for RefundReturnCommand {
    type Result = RefundReturnResult;

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

        let refunded_return = self.refund_return(db).await?;

        self.log_and_trigger_event(&event_sender, &refunded_return)
            .await?;

        Ok(RefundReturnResult {
            id: refunded_return.id,
            status: refunded_return.status,
            refund_amount: self.refund_amount,
        })
    }
}

impl RefundReturnCommand {
    async fn refund_return(
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
        return_request.status = Set(ReturnStatus::Refunded.as_str().to_owned());

        let refunded_return = return_request.update(db).await.map_err(|e| {
            let msg = format!("Failed to update return {}: {}", self.return_id, e);
            error!("{}", msg);
            ServiceError::db_error(e)
        })?;

        // Assume refund processing logic is here
        info!(
            "Refund processed for return ID: {}. Amount: {}",
            self.return_id, self.refund_amount
        );

        Ok(refunded_return)
    }

    async fn log_and_trigger_event(
        &self,
        event_sender: &EventSender,
        refunded_return: &return_entity::Model,
    ) -> Result<(), ServiceError> {
        info!(
            "Return refunded for return ID: {}. Amount: {}",
            self.return_id, self.refund_amount
        );
        event_sender
            .send(Event::ReturnUpdated(self.return_id))
            .await
            .map_err(|e| {
                let msg = format!("Failed to send event for refunded return: {}", e);
                error!("{}", msg);
                ServiceError::EventError(msg)
            })
    }
}
