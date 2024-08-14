use std::sync::Arc;
use crate::{errors::ServiceError, db::DbPool, models::{Return, ReturnStatus}};
use crate::events::{Event, EventSender};
use tracing::{info, error, instrument};
use diesel::prelude::*;
use serde::{Serialize, Deserialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct CompleteReturnCommand {
    pub return_id: i32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CompleteReturnResult {
    pub id: String,
    pub object: String,
    pub completed: bool,
}

#[async_trait::async_trait]
impl Command for CompleteReturnCommand {
    type Result = CompleteReturnResult;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|e| {
            error!("Failed to get database connection: {}", e);
            ServiceError::DatabaseError("Failed to get database connection".into())
        })?;

        let completed_return = conn.transaction(|| {
            self.complete_return(&conn)
        }).map_err(|e| {
            error!("Transaction failed for completing return ID {}: {}", self.return_id, e);
            e
        })?;

        self.log_and_trigger_event(event_sender, &completed_return).await?;

        Ok(CompleteReturnResult {
            id: completed_return.id.to_string(),
            object: "return".to_string(),
            completed: true,
        })
    }
}

impl CompleteReturnCommand {
    fn complete_return(&self, conn: &PgConnection) -> Result<Return, ServiceError> {
        diesel::update(returns::table.find(self.return_id))
            .set(returns::status.eq(ReturnStatus::Completed))
            .get_result::<Return>(conn)
            .map_err(|e| {
                if e == diesel::result::Error::NotFound {
                    error!("Return request not found: {}", self.return_id);
                    ServiceError::NotFound(format!("Return request with ID {} not found", self.return_id))
                } else {
                    error!("Failed to complete return request: {}", e);
                    ServiceError::DatabaseError(format!("Failed to complete return request: {}", e))
                }
            })
    }

    async fn log_and_trigger_event(&self, event_sender: Arc<EventSender>, completed_return: &Return) -> Result<(), ServiceError> {
        info!("Return request completed for return ID: {}", self.return_id);
        event_sender.send(Event::ReturnCompleted(self.return_id))
            .await
            .map_err(|e| {
                error!("Failed to send ReturnCompleted event for return ID {}: {}", self.return_id, e);
                ServiceError::EventError(e.to_string())
            })
    }
}
