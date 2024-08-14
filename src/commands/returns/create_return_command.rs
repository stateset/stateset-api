use std::sync::Arc;
use crate::{errors::ServiceError, db::DbPool, models::{Return, ReturnStatus}};
use crate::events::{Event, EventSender};
use validator::Validate;
use tracing::{info, error, instrument};
use diesel::prelude::*;

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct InitiateReturnCommand {
    pub order_id: i32,
    
    #[validate(length(min = 1))]
    pub reason: String,
}

#[async_trait::async_trait]
impl Command for InitiateReturnCommand {
    type Result = Return;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|e| {
            error!("Failed to get database connection: {}", e);
            ServiceError::DatabaseError("Failed to get database connection".into())
        })?;

        let saved_return = self.create_return_request(&conn)?;

        self.log_and_trigger_event(event_sender, &saved_return).await?;

        Ok(saved_return)
    }
}

impl InitiateReturnCommand {
    fn create_return_request(&self, conn: &PgConnection) -> Result<Return, ServiceError> {
        let return_request = Return {
            order_id: self.order_id,
            reason: self.reason.clone(),
            status: ReturnStatus::Pending,
            // Add other fields as necessary, like created_at, updated_at, etc.
        };

        diesel::insert_into(returns::table)
            .values(&return_request)
            .get_result::<Return>(conn)
            .map_err(|e| {
                error!("Failed to initiate return for order ID {}: {}", self.order_id, e);
                ServiceError::DatabaseError(format!("Failed to initiate return: {}", e))
            })
    }

    async fn log_and_trigger_event(&self, event_sender: Arc<EventSender>, saved_return: &Return) -> Result<(), ServiceError> {
        info!("Return initiated for order ID: {}. Reason: {}", self.order_id, self.reason);
        event_sender.send(Event::ReturnInitiated(saved_return.id))
            .await
            .map_err(|e| {
                error!("Failed to send ReturnInitiated event for return ID {}: {}", saved_return.id, e);
                ServiceError::EventError(e.to_string())
            })
    }
}
