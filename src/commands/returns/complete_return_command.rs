use std::sync::Arc;
use crate::{errors::ServiceError, db::DbPool, models::{Return, ReturnStatus}};
use crate::events::{Event, EventSender};
use validator::Validate;
use tracing::info;
use chrono::{DateTime, Utc};
use diesel::prelude::*;

#[derive(Debug, Serialize, Deserialize)]
pub struct CompleteReturnCommand {
    pub return_id: i32,
}

#[async_trait]
impl Command for CompleteReturnCommand {
    type Result = Return;

    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;

        let result = conn.transaction::<_, ServiceError, _>(|| {
            // Update the return status to Completed
            let completed_return = diesel::update(returns::table.find(self.return_id))
                .set(returns::status.eq(ReturnStatus::Completed))
                .get_result::<Return>(&conn)
                .map_err(|e| ServiceError::DatabaseError(format!("Failed to complete return: {}", e)))?;

            // Trigger an event
            event_sender.send(Event::ReturnCompleted(self.return_id))
                .await
                .map_err(|e| ServiceError::EventError(e.to_string()))?;

            Ok(completed_return)
        })?;

        Ok(result)
    }
}
