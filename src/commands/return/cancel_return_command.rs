use std::sync::Arc;
use crate::{errors::ServiceError, db::DbPool, models::{Return, ReturnStatus}};
use crate::events::{Event, EventSender};
use validator::Validate;
use tracing::info;
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

    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;

        let result = conn.transaction::<_, ServiceError, _>(|| {
            // Find the return request and update its status to Cancelled
            let cancelled_return = diesel::update(returns::table.find(self.return_id))
                .set((
                    returns::status.eq(ReturnStatus::Cancelled),
                    returns::reason.eq(self.reason.clone()),
                ))
                .get_result::<Return>(&conn)
                .map_err(|e| {
                    if e == diesel::result::Error::NotFound {
                        ServiceError::NotFound("Return request not found".into())
                    } else {
                        ServiceError::DatabaseError(format!("Failed to cancel return request: {}", e))
                    }
                })?;

            // Log the cancellation and trigger events
            info!("Return request cancelled for return ID: {}. Reason: {}", self.return_id, self.reason);
            event_sender.send(Event::ReturnCancelled(self.return_id))
                .await
                .map_err(|e| ServiceError::EventError(e.to_string()))?;

            Ok(cancelled_return)
        })?;

        Ok(result)
    }
}
