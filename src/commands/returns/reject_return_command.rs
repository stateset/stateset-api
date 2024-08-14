use std::sync::Arc;
use crate::{errors::ServiceError, db::DbPool, models::{Return, ReturnStatus}};
use crate::events::{Event, EventSender};
use validator::Validate;
use tracing::{info, error, instrument};
use diesel::prelude::*;
use async_trait::async_trait;
use serde::{Serialize, Deserialize};

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct RejectReturnCommand {
    pub return_id: i32,

    #[validate(length(min = 1))]
    pub reason: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RejectReturnResult {
    pub id: String,
    pub object: String,
    pub rejected: bool,
    pub reason: String,
}

#[async_trait]
impl Command for RejectReturnCommand {
    type Result = RejectReturnResult;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|e| {
            error!("Failed to get database connection: {}", e);
            ServiceError::DatabaseError("Failed to get database connection".into())
        })?;

        let rejected_return = conn.transaction(|| {
            self.reject_return(&conn)
        }).map_err(|e| {
            error!("Transaction failed for rejecting return ID {}: {}", self.return_id, e);
            e
        })?;

        self.log_and_trigger_event(event_sender, &rejected_return).await?;

        Ok(RejectReturnResult {
            id: rejected_return.id.to_string(),
            object: "return".to_string(),
            rejected: true,
            reason: self.reason.clone(),
        })
    }
}

impl RejectReturnCommand {
    fn reject_return(&self, conn: &PgConnection) -> Result<Return, ServiceError> {
        diesel::update(returns::table.find(self.return_id))
            .set((
                returns::status.eq(ReturnStatus::Rejected),
                returns::reason.eq(self.reason.clone()),
            ))
            .get_result::<Return>(conn)
            .map_err(|e| {
                if e == diesel::result::Error::NotFound {
                    error!("Return request not found: {}", self.return_id);
                    ServiceError::NotFound(format!("Return request with ID {} not found", self.return_id))
                } else {
                    error!("Failed to reject return request: {}", e);
                    ServiceError::DatabaseError(format!("Failed to reject return request: {}", e))
                }
            })
    }

    async fn log_and_trigger_event(&self, event_sender: Arc<EventSender>, rejected_return: &Return) -> Result<(), ServiceError> {
        info!("Return request rejected for return ID: {}. Reason: {}", self.return_id, self.reason);
        event_sender.send(Event::ReturnRejected(self.return_id))
            .await
            .map_err(|e| {
                error!("Failed to send ReturnRejected event for return ID {}: {}", self.return_id, e);
                ServiceError::EventError(e.to_string())
            })
    }
}
