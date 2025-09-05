use crate::{
    commands::Command,
    db::DbPool,
    errors::ServiceError,
    events::{Event, EventSender},
    models::{
        r#return::ReturnStatus,
        return_entity::{self, Entity as Return, Model as ReturnEntity},
        return_note_entity::{self, Entity as ReturnNote},
        return_history_entity::{self, Entity as ReturnHistory},
    },
};
use async_trait::async_trait;
use chrono::Utc;
use sea_orm::{
    entity::*,
    query::*,
    DatabaseConnection, DatabaseTransaction, Set, TransactionError, TransactionTrait,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{debug, error, info, instrument, warn};
use uuid::Uuid;
use validator::{Validate, ValidationError};

/// Command to close a return
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloseReturnCommand {
    /// Unique identifier of the return to be closed
    pub return_id: Uuid,
    /// Optional reason for closing the return
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// Optional additional notes about the closure
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    /// User or system ID that initiated the closure
    #[serde(skip_serializing_if = "Option::is_none")]
    pub closed_by: Option<String>,
    /// Additional metadata about the closure
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

/// Result returned after closing a return
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloseReturnResult {
    /// Unique identifier of the closed return
    pub id: Uuid,
    /// Object type indicator
    pub object: String,
    /// Flag indicating successful closure
    pub closed: bool,
    /// Timestamp when the return was closed
    pub closed_at: String,
    /// User or system that closed the return
    #[serde(skip_serializing_if = "Option::is_none")]
    pub closed_by: Option<String>,
    /// Reason for closing the return
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[async_trait::async_trait]
impl crate::commands::Command for CloseReturnCommand {
    type Result = CloseReturnResult;

    #[instrument(skip(self, db_pool, event_sender), fields(return_id = %self.return_id))]
    async fn execute(
        &self,
        db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        debug!("Executing CloseReturnCommand");
        let db = db_pool.as_ref();

        // Capture needed data to avoid borrowing `&self` across 'static closure
        let return_id = self.return_id;
        let reason = self.reason.clone();
        let notes = self.notes.clone();
        let closed_by = self.closed_by.clone();
        let metadata = self.metadata.clone();

        // Execute the operation inside a transaction
        let result = db
            .transaction::<_, Self::Result, ServiceError>(move |txn| {
                let reason = reason.clone();
                let notes = notes.clone();
                let closed_by = closed_by.clone();
                let metadata = metadata.clone();
                Box::pin(async move {
                    // Check if return exists and can be closed
                    let return_request = Self::validate_return_state_static(return_id, txn).await?;

                    // Update return status to closed
                    let updated_return = Self::close_return_static(return_id, reason.as_ref(), closed_by.as_ref(), metadata.as_ref(), txn, &return_request).await?;

                    // Add closure note if provided
                    if let Some(note) = &notes {
                        Self::add_closure_note_static(return_id, closed_by.as_ref(), txn, note).await?;
                    }

                    // Create history record
                    Self::create_history_record_static(return_id, reason.as_ref(), metadata.as_ref(), txn, &return_request).await?;

                    Ok(CloseReturnResult {
                        id: Uuid::parse_str(&updated_return.id).unwrap_or_else(|_| Uuid::new_v4()),
                        object: "return".to_string(),
                        closed: true,
                        closed_at: updated_return
                            .updated_at
                            .format("%Y-%m-%dT%H:%M:%S%.3fZ")
                            .to_string(),
                        closed_by: closed_by.clone(),
                        reason: reason.clone(),
                    })
                })
            })
            .await;

        match result {
            Ok(result) => {
                // Log and trigger event outside the transaction
                self.log_and_trigger_event(&event_sender, &result).await?;
                Ok(result)
            }
            Err(e) => {
                error!("Failed to close return: {}", e);
                match e {
                    TransactionError::Connection(db_err) => Err(ServiceError::DatabaseError(db_err)),
                    TransactionError::Transaction(service_err) => Err(service_err),
                }
            }
        }
    }
}

impl CloseReturnCommand {
    async fn validate_return_state_static(
        return_id: Uuid,
        db: &DatabaseTransaction,
    ) -> Result<ReturnEntity, ServiceError> {
        let return_request = Return::find_by_id(return_id)
            .one(db)
            .await
            .map_err(|e| {
                let msg = format!("Database error when finding return: {}", e);
                error!(error = %e, return_id = %return_id, "{}", msg);
                ServiceError::DatabaseError(msg)
            })?
            .ok_or_else(|| {
                let msg = format!("Return with ID {} not found", return_id);
                warn!(return_id = %return_id, "{}", msg);
                ServiceError::NotFound(msg)
            })?;

        let current_status = return_request.status.clone();
        if current_status == ReturnStatus::Cancelled.as_str() || current_status == "Closed" {
            let msg = format!(
                "Cannot close return in state {}. Return is already in a terminal state.",
                current_status,
            );
            warn!(current_status = %current_status, return_id = %return_id, "{}", msg);
            return Err(ServiceError::InvalidStatus(msg));
        }

        Ok(return_request)
    }

    async fn close_return_static(
        return_id: Uuid,
        reason: Option<&String>,
        _closed_by: Option<&String>,
        _metadata: Option<&serde_json::Value>,
        db: &DatabaseTransaction,
        return_request: &ReturnEntity,
    ) -> Result<ReturnEntity, ServiceError> {
        let now = Utc::now().naive_utc();
        let mut return_active: return_entity::ActiveModel = return_request.clone().into();
        return_active.status = Set("Closed".to_string());
        return_active.updated_at = Set(now);

        let updated_return = return_active.update(db).await.map_err(|e| {
            let msg = format!("Failed to update return status to Closed: {}", e);
            error!(error = %e, return_id = %return_id, "{}", msg);
            ServiceError::DatabaseError(msg)
        })?;

        debug!(return_id = %return_id, "Return status updated to Closed");
        Ok(updated_return)
    }

    async fn add_closure_note_static(
        return_id: Uuid,
        _closed_by: Option<&String>,
        db: &DatabaseTransaction,
        note_text: &str,
    ) -> Result<(), ServiceError> {
        let note_content = format!("Return closed. Note: {}", note_text);

        let note = return_note_entity::ActiveModel {
            id: Set(Uuid::new_v4()),
            return_id: Set(return_id),
            note_type: Set(return_note_entity::ReturnNoteType::System),
            content: Set(note_content),
            created_by: Set(None),
            is_visible_to_customer: Set(false),
            created_at: Set(Utc::now()),
            updated_at: Set(Utc::now()),
            ..Default::default()
        };

        ReturnNote::insert(note).exec(db).await.map_err(|e| {
            let msg = format!("Failed to add closure note: {}", e);
            error!(error = %e, return_id = %return_id, "{}", msg);
            ServiceError::DatabaseError(msg)
        })?;

        debug!(return_id = %return_id, "Added closure note");
        Ok(())
    }

    async fn create_history_record_static(
        return_id: Uuid,
        reason: Option<&String>,
        _metadata: Option<&serde_json::Value>,
        db: &DatabaseTransaction,
        return_request: &ReturnEntity,
    ) -> Result<(), ServiceError> {
        let history = return_history_entity::ActiveModel {
            id: Set(Uuid::new_v4()),
            return_id: Set(return_id),
            status_from: Set(return_request.status.clone()),
            status_to: Set("Closed".to_string()),
            changed_by: Set(None),
            change_reason: Set(reason.cloned()),
            notes: Set(None),
            created_at: Set(Utc::now()),
            ..Default::default()
        };

        ReturnHistory::insert(history).exec(db).await.map_err(|e| {
            let msg = format!("Failed to create history record: {}", e);
            error!(error = %e, return_id = %return_id, "{}", msg);
            ServiceError::DatabaseError(msg)
        })?;

        debug!(return_id = %return_id, "Created history record for closure");
        Ok(())
    }

    /// Logs the closure and triggers related events
    async fn log_and_trigger_event(
        &self,
        event_sender: &EventSender,
        _result: &CloseReturnResult,
    ) -> Result<(), ServiceError> {
        info!(
            return_id = %self.return_id,
            closed_by = ?self.closed_by,
            reason = ?self.reason,
            "Return request successfully closed"
        );

        event_sender
            .send(Event::ReturnUpdated(self.return_id))
            .await
            .map_err(|e| {
                let msg = format!("Failed to send ReturnUpdated event: {}", e);
                error!(error = %e, return_id = %self.return_id, "{}", msg);
                ServiceError::EventError(msg)
            })
    }
}
