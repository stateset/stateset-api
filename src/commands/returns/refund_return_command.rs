use std::sync::Arc;
use sea_orm::*;
use crate::{errors::ServiceError, db::DbPool, models::{return_entity, return_entity::Entity as Return}};
use crate::models::return_entity::ReturnStatus;
use crate::events::{Event, EventSender};
use tracing::{info, error, instrument};
use serde::{Serialize, Deserialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct RefundReturnCommand {
    pub return_id: i32,
    pub refund_amount: f64,
}

#[async_trait::async_trait]
impl Command for RefundReturnCommand {
    type Result = return_entity::Model;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        let db = db_pool.get().map_err(|e| {
            error!("Failed to get database connection: {}", e);
            ServiceError::DatabaseError("Failed to get database connection".into())
        })?;

        let refunded_return = self.refund_return(&db).await?;

        self.log_and_trigger_event(event_sender, &refunded_return).await?;

        Ok(refunded_return)
    }
}

impl RefundReturnCommand {
    async fn refund_return(&self, db: &DatabaseConnection) -> Result<return_entity::Model, ServiceError> {
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

        let mut return_request: return_entity::ActiveModel = return_request.into();
        return_request.status = Set(ReturnStatus::Refunded.to_string());

        let refunded_return = return_request
            .update(db)
            .await
            .map_err(|e| {
                error!("Failed to refund return ID {}: {}", self.return_id, e);
                ServiceError::DatabaseError(format!("Failed to refund return: {}", e))
            })?;

        // Assume refund processing logic is here
        info!("Refund processed for return ID: {}. Amount: {}", self.return_id, self.refund_amount);

        Ok(refunded_return)
    }

    async fn log_and_trigger_event(&self, event_sender: Arc<EventSender>, refunded_return: &return_entity::Model) -> Result<(), ServiceError> {
        event_sender.send(Event::ReturnRefunded(self.return_id))
            .await
            .map_err(|e| {
                error!("Failed to send ReturnRefunded event for return ID {}: {}", self.return_id, e);
                ServiceError::EventError(e.to_string())
            })
    }
}