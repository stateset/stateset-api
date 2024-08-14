use std::sync::Arc;
use crate::{errors::ServiceError, db::DbPool, models::{Return, ReturnStatus}};
use crate::events::{Event, EventSender};
use validator::Validate;
use tracing::{error, info, instrument};
use chrono::{DateTime, Utc};
use diesel::prelude::*;

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct CancelReturnCommand {
    pub return_id: i32,
    
    #[validate(length(min = 1))]
    pub reason: String,
}

#[async_trait::async_trait]
impl Command for CancelReturnCommand {
    type Result = Return;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|e| {
            error!("Failed to get database connection: {}", e);
            ServiceError::DatabaseError("Failed to get database connection".into())
        })?;

        let cancelled_return = conn.transaction::<Return, ServiceError, _>(|| {
            self.cancel_return(&conn)
        }).map_err(|e| {
            error!("Transaction failed for cancelling return ID {}: {}", self.return_id, e);
            e
        })?;

        self.log_and_trigger_event(event_sender, &cancelled_return).await?;

        Ok(cancelled_return)
    }
}

impl CancelReturnCommand {
    fn cancel_return(&self, conn: &PgConnection) -> Result<Return, ServiceError> {
        diesel::update(returns::table.find(self.return_id))
            .set((
                returns::status.eq(ReturnStatus::Cancelled),
                returns::reason.eq(self.reason.clone()),
            ))
            .get_result::<Return>(conn)
            .map_err(|e| {
                if e == diesel::result::Error::NotFound {
                    error!("Return request not found: {}", self.return_id);
                    ServiceError::NotFound(format!("Return request with ID {} not found", self.return_id))
                } else {
                    error!("Failed to cancel return request: {}", e);
                    ServiceError::DatabaseError(format!("Failed to cancel return request: {}", e))
                }
            })
    }

    async fn log_and_trigger_event(&self, event_sender: Arc<EventSender>, cancelled_return: &Return) -> Result<(), ServiceError> {
        info!("Return request cancelled for return ID: {}. Reason: {}", self.return_id, self.reason);
        event_sender.send(Event::ReturnCancelled(self.return_id))
            .await
            .map_err(|e| {
                error!("Failed to send ReturnCancelled event for return ID {}: {}", self.return_id, e);
                ServiceError::EventError(e.to_string())
            })
    }
}
