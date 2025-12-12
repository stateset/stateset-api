use crate::{
    db::DbPool,
    errors::ServiceError,
    events::{Event, EventSender},
    models::{
        return_entity::{self, Entity as Return, Model as ReturnEntity},
        return_history_entity::{self, Entity as ReturnHistory},
        return_note_entity::{self, Entity as ReturnNote},
    },
};
use async_trait::async_trait;
use chrono::Utc;
use sea_orm::{entity::*, DatabaseTransaction, Set, TransactionError, TransactionTrait};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::error;
use uuid::Uuid;

/// Command to mark a return as completed
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompleteReturnCommand {
    /// Unique identifier of the return to be completed
    pub return_id: Uuid,
    /// Optional notes about the completion
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    /// User or system ID that initiated the completion
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_by: Option<String>,
    /// Additional metadata about the completion
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

/// Result returned after completing a return
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompleteReturnResult {
    /// Unique identifier of the completed return
    pub id: Uuid,
    /// Object type indicator
    pub object: String,
    /// Flag indicating successful completion
    pub completed: bool,
    /// Timestamp when the return was completed
    pub completed_at: String,
    /// User or system that completed the return
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_by: Option<String>,
}

#[async_trait]
impl crate::commands::Command for CompleteReturnCommand {
    type Result = CompleteReturnResult;

    #[tracing::instrument(skip(self, db_pool, event_sender), fields(return_id = %self.return_id))]
    async fn execute(
        &self,
        db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        tracing::debug!("Executing CompleteReturnCommand");
        let db = db_pool.as_ref();

        // Capture data to avoid borrowing `&self` across 'static closure
        let return_id = self.return_id;
        let notes = self.notes.clone();
        let completed_by = self.completed_by.clone();
        let metadata = self.metadata.clone();

        // Execute the operation inside a transaction
        let result = db
            .transaction::<_, Self::Result, ServiceError>(move |txn| {
                let notes = notes.clone();
                let completed_by = completed_by.clone();
                let metadata = metadata.clone();
                Box::pin(async move {
                    // Check if return exists and can be completed
                    let return_request = Self::validate_return_state_static(return_id, txn).await?;

                    // Update return status to completed
                    let updated_return = Self::complete_return_static(
                        return_id,
                        metadata.as_ref(),
                        txn,
                        &return_request,
                    )
                    .await?;

                    // Add completion note if provided
                    if let Some(note) = &notes {
                        Self::add_completion_note_static(
                            return_id,
                            completed_by.as_ref(),
                            txn,
                            note,
                        )
                        .await?;
                    }

                    // Create history record
                    Self::create_history_record_static(
                        return_id,
                        completed_by.as_ref(),
                        notes.as_ref(),
                        txn,
                        &return_request,
                    )
                    .await?;

                    // Enqueue outbox within the same transaction
                    let payload = serde_json::json!({
                        "return_id": return_id.to_string(),
                        "completed_by": completed_by,
                    });
                    let _ = crate::events::outbox::enqueue(
                        txn,
                        "return",
                        Some(return_id),
                        "ReturnCompleted",
                        &payload,
                    )
                    .await;

                    Ok(CompleteReturnResult {
                        id: updated_return.id,
                        object: "return".to_string(),
                        completed: true,
                        completed_at: updated_return
                            .updated_at
                            .format("%Y-%m-%dT%H:%M:%S%.3fZ")
                            .to_string(),
                        completed_by: completed_by.clone(),
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
                error!("Failed to complete return: {}", e);
                match e {
                    TransactionError::Connection(db_err) => Err(ServiceError::db_error(db_err)),
                    TransactionError::Transaction(service_err) => Err(service_err),
                }
            }
        }
    }
}

impl CompleteReturnCommand {
    async fn validate_return_state_static(
        return_id: Uuid,
        db: &DatabaseTransaction,
    ) -> Result<ReturnEntity, ServiceError> {
        let return_request = Return::find_by_id(return_id)
            .one(db)
            .await
            .map_err(|e| {
                let msg = format!("Database error when finding return: {}", e);
                tracing::error!(error = %e, return_id = %return_id, "{}", msg);
                ServiceError::db_error(msg)
            })?
            .ok_or_else(|| {
                let msg = format!("Return with ID {} not found", return_id);
                tracing::warn!(return_id = %return_id, "{}", msg);
                ServiceError::NotFound(msg)
            })?;

        let current_status = return_request.status.clone();
        let valid_previous_states = ["Approved", "Received"]; // using allowed prior statuses
        if !valid_previous_states.contains(&current_status.as_str()) {
            let msg = format!(
                "Cannot complete return in state {}. Return must be in one of the following states: {:?}",
                current_status,
                valid_previous_states
            );
            tracing::warn!(
                current_status = %current_status,
                return_id = %return_id,
                valid_states = ?valid_previous_states,
                "{}", msg
            );
            return Err(ServiceError::InvalidStatus(msg));
        }

        Ok(return_request)
    }

    async fn complete_return_static(
        _return_id: Uuid,
        metadata: Option<&serde_json::Value>,
        db: &DatabaseTransaction,
        return_request: &ReturnEntity,
    ) -> Result<ReturnEntity, ServiceError> {
        let now = Utc::now().naive_utc();
        let mut return_active: return_entity::ActiveModel = return_request.clone().into();

        return_active.status = Set("Completed".to_string());
        return_active.updated_at = Set(now);

        if let Some(metadata) = metadata {
            if let serde_json::Value::Object(new_data) = metadata {
                // The `return_entity::Model` doesn't have a metadata field in this codebase;
                // skipping persistence and only updating status/timestamps.
                let _ = new_data; // avoid lint
            }
        }

        let updated_return = return_active.update(db).await.map_err(|e| {
            let msg = format!("Failed to update return status to Completed: {}", e);
            tracing::error!(error = %e, "{}", msg);
            ServiceError::db_error(msg)
        })?;

        tracing::debug!("Return status updated to Completed");
        Ok(updated_return)
    }

    async fn add_completion_note_static(
        return_id: Uuid,
        completed_by: Option<&String>,
        txn: &DatabaseTransaction,
        note_text: &str,
    ) -> Result<(), ServiceError> {
        let note = return_note_entity::ActiveModel {
            id: Set(Uuid::new_v4()),
            return_id: Set(return_id),
            note_type: Set(return_note_entity::ReturnNoteType::System),
            content: Set(note_text.to_string()),
            created_by: Set(completed_by.and_then(|s| Uuid::parse_str(s).ok())),
            is_visible_to_customer: Set(false),
            created_at: Set(Utc::now()),
            updated_at: Set(Utc::now()),
            ..Default::default()
        };

        ReturnNote::insert(note).exec(txn).await.map_err(|e| {
            error!("Failed to add completion note: {}", e);
            ServiceError::db_error(format!("Failed to add completion note: {}", e))
        })?;

        Ok(())
    }

    async fn create_history_record_static(
        return_id: Uuid,
        completed_by: Option<&String>,
        _notes: Option<&String>,
        txn: &DatabaseTransaction,
        return_request: &ReturnEntity,
    ) -> Result<(), ServiceError> {
        let history = return_history_entity::ActiveModel {
            id: Set(Uuid::new_v4()),
            return_id: Set(return_id),
            status_from: Set(return_request.status.clone()),
            status_to: Set("Completed".to_string()),
            changed_by: Set(completed_by.and_then(|s| Uuid::parse_str(s).ok())),
            change_reason: Set(Some("Return completed".to_string())),
            notes: Set(None),
            created_at: Set(Utc::now()),
            ..Default::default()
        };

        ReturnHistory::insert(history)
            .exec(txn)
            .await
            .map_err(|e| {
                let msg = format!("Failed to create history record: {}", e);
                tracing::error!(error = %e, return_id = %return_id, "{}", msg);
                ServiceError::db_error(msg)
            })?;

        tracing::debug!(return_id = %return_id, "Created history record for completion");
        Ok(())
    }

    async fn log_and_trigger_event(
        &self,
        event_sender: &EventSender,
        _result: &CompleteReturnResult,
    ) -> Result<(), ServiceError> {
        tracing::info!(
            return_id = %self.return_id,
            completed_by = ?self.completed_by,
            "Return request successfully completed"
        );

        event_sender
            .send(Event::ReturnUpdated(self.return_id))
            .await
            .map_err(|e| {
                let msg = format!("Failed to send ReturnUpdated event: {}", e);
                tracing::error!(error = %e, return_id = %self.return_id, "{}", msg);
                ServiceError::EventError(msg)
            })
    }
}
