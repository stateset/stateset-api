use std::sync::Arc;
use crate::{errors::ServiceError, db::DbPool, models::{Return, ReturnStatus}};
use crate::events::{Event, EventSender};
use validator::Validate;
use tracing::info;
use diesel::prelude::*;
use async_trait::async_trait;

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct RejectReturnCommand {
    pub return_id: i32,
    #[validate(length(min = 1))]
    pub reason: String,
}

#[async_trait]
impl Command for RejectReturnCommand {
    type Result = Return;

    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;

        let result = conn.transaction::<_, ServiceError, _>(|| {
            // Find the return request and update its status to Rejected
            let rejected_return = diesel::update(returns::table.find(self.return_id))
                .set((
                    returns::status.eq(ReturnStatus::Rejected),
                    returns::reason.eq(self.reason.clone()),
                ))
                .get_result::<Return>(&conn)
                .map_err(|e| {
                    if e == diesel::result::Error::NotFound {
                        ServiceError::NotFound("Return request not found".into())
                    } else {
                        ServiceError::DatabaseError(format!("Failed to reject return request: {}", e))
                    }
                })?;

            // Log the rejection and trigger events
            info!("Return request rejected for return ID: {}. Reason: {}", self.return_id, self.reason);
            event_sender.send(Event::ReturnRejected(self.return_id))
                .await
                .map_err(|e| ServiceError::EventError(e.to_string()))?;

            Ok(rejected_return)
        })?;

        Ok(result)
    }
}
