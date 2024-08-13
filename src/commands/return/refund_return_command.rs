use std::sync::Arc;
use crate::{errors::ServiceError, db::DbPool, models::{Return, ReturnStatus}};
use crate::events::{Event, EventSender};
use validator::Validate;
use tracing::info;
use chrono::{DateTime, Utc};


#[derive(Debug, Serialize, Deserialize)]
pub struct RefundReturnCommand {
    pub return_id: i32,
    pub refund_amount: f64,
}

#[async_trait]
impl Command for RefundReturnCommand {
    type Result = Return;

    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;

        let result = conn.transaction::<_, ServiceError, _>(|| {
            // Update the return status to Refunded
            let refunded_return = diesel::update(returns::table.find(self.return_id))
                .set(returns::status.eq(ReturnStatus::Refunded))
                .get_result::<Return>(&conn)
                .map_err(|e| ServiceError::DatabaseError(format!("Failed to refund return: {}", e)))?;

            // Log and process the refund
            // Assume refund processing logic is here
            info!("Refund processed for return ID: {}. Amount: {}", self.return_id, self.refund_amount);

            // Trigger an event
            event_sender.send(Event::ReturnRefunded(self.return_id))
                .await
                .map_err(|e| ServiceError::EventError(e.to_string()))?;

            Ok(refunded_return)
        })?;

        Ok(result)
    }
}
