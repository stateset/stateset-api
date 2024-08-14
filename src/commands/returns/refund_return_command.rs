use std::sync::Arc;
use crate::{errors::ServiceError, db::DbPool, models::{Return, ReturnStatus}};
use crate::events::{Event, EventSender};
use tracing::{info, error, instrument};
use diesel::prelude::*;

#[derive(Debug, Serialize, Deserialize)]
pub struct RefundReturnCommand {
    pub return_id: i32,
    pub refund_amount: f64,
}

#[async_trait::async_trait]
impl Command for RefundReturnCommand {
    type Result = Return;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|e| {
            error!("Failed to get database connection: {}", e);
            ServiceError::DatabaseError("Failed to get database connection".into())
        })?;

        let refunded_return = conn.transaction(|| {
            self.refund_return(&conn)
        }).map_err(|e| {
            error!("Transaction failed for refunding return ID {}: {}", self.return_id, e);
            e
        })?;

        self.log_and_trigger_event(event_sender, &refunded_return).await?;

        Ok(refunded_return)
    }
}

impl RefundReturnCommand {
    fn refund_return(&self, conn: &PgConnection) -> Result<Return, ServiceError> {
        let refunded_return = diesel::update(returns::table.find(self.return_id))
            .set(returns::status.eq(ReturnStatus::Refunded))
            .get_result::<Return>(conn)
            .map_err(|e| {
                error!("Failed to refund return ID {}: {}", self.return_id, e);
                ServiceError::DatabaseError(format!("Failed to refund return: {}", e))
            })?;

        // Assume refund processing logic is here
        info!("Refund processed for return ID: {}. Amount: {}", self.return_id, self.refund_amount);

        Ok(refunded_return)
    }

    async fn log_and_trigger_event(&self, event_sender: Arc<EventSender>, refunded_return: &Return) -> Result<(), ServiceError> {
        event_sender.send(Event::ReturnRefunded(self.return_id))
            .await
            .map_err(|e| {
                error!("Failed to send ReturnRefunded event for return ID {}: {}", self.return_id, e);
                ServiceError::EventError(e.to_string())
            })
    }
}
