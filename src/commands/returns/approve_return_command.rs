use std::sync::Arc;
use crate::{errors::ServiceError, db::DbPool, models::{Return, ReturnStatus}};
use crate::events::{Event, EventSender};
use validator::Validate;
use tracing::{info, error, instrument};
use chrono::{DateTime, Utc};
use diesel::prelude::*;

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct ApproveReturnCommand {
    pub return_id: i32,
}

#[async_trait::async_trait]
impl Command for ApproveReturnCommand {
    type Result = Return;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|e| {
            error!("Database connection error: {}", e);
            ServiceError::DatabaseError("Failed to get database connection".into())
        })?;

        let approved_return = conn.transaction(|| {
            self.approve_return(&conn)
        }).map_err(|e| {
            error!("Transaction failed for approving return ID {}: {}", self.return_id, e);
            e
        })?;

        self.log_and_trigger_event(event_sender, &approved_return).await?;

        Ok(approved_return)
    }
}

impl ApproveReturnCommand {
    fn approve_return(&self, conn: &PgConnection) -> Result<Return, ServiceError> {
        diesel::update(returns::table.find(self.return_id))
            .set(returns::status.eq(ReturnStatus::Approved))
            .get_result::<Return>(conn)
            .map_err(|e| {
                if let diesel::result::Error::NotFound = e {
                    error!("Return request not found: {}", self.return_id);
                    ServiceError::NotFound(format!("Return request with ID {} not found", self.return_id))
                } else {
                    error!("Failed to approve return request: {}", e);
                    ServiceError::DatabaseError(format!("Database error: {}", e))
                }
            })
    }

    async fn log_and_trigger_event(&self, event_sender: Arc<EventSender>, approved_return: &Return) -> Result<(), ServiceError> {
        info!("Return request approved for return ID: {}", self.return_id);
        event_sender.send(Event::ReturnApproved(self.return_id))
            .await
            .map_err(|e| {
                error!("Failed to send event for approved return: {}", e);
                ServiceError::EventError(e.to_string())
            })
    }
}
