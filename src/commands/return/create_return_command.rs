use std::sync::Arc;
use crate::{errors::ServiceError, db::DbPool, models::{Return, ReturnStatus}};
use crate::events::{Event, EventSender};
use validator::Validate;
use tracing::info;
use chrono::{DateTime, Utc};


#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct InitiateReturnCommand {
    pub order_id: i32,
    #[validate(length(min = 1))]
    pub reason: String,
}

#[async_trait]
impl Command for InitiateReturnCommand {
    type Result = Return;

    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;

        // Create a new return request
        let return_request = Return {
            order_id: self.order_id,
            reason: self.reason.clone(),
            status: ReturnStatus::Pending,
            // Other fields like created_at, updated_at, etc.
        };

        let saved_return = diesel::insert_into(returns::table)
            .values(&return_request)
            .get_result::<Return>(&conn)
            .map_err(|e| ServiceError::DatabaseError)?;

        // Log and trigger events
        info!("Return initiated for order ID: {}. Reason: {}", self.order_id, self.reason);
        event_sender.send(Event::ReturnInitiated(saved_return.id)).await.map_err(|e| ServiceError::EventError(e.to_string()))?;

        Ok(saved_return)
    }
}