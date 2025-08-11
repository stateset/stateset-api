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
use tracing::{error, info};
use uuid::Uuid;
use validator::{Validate, ValidationError};

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

        // Execute the operation inside a transaction
        let result = db
            .transaction::<_, Self::Result, ServiceError>(|txn| {
                Box::pin(async move {
                    // Check if return exists and can be completed
                    let return_request = self.validate_return_state(txn).await?;

                    // Update return status to completed
                    let updated_return = self.complete_return(txn, &return_request).await?;

                    // Add completion note if provided
                    if let Some(note) = &self.notes {
                        self.add_completion_note(txn, note).await?;
                    }

                    // Create history record
                    self.create_history_record(txn, &return_request).await?;

                    Ok(CompleteReturnResult {
                        id: Uuid::parse_str(&updated_return.id).unwrap_or_else(|_| Uuid::new_v4()),
                        object: "return".to_string(),
                        completed: true,
                        completed_at: updated_return.updated_at.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string(),
                        completed_by: self.completed_by.clone(),
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
                    TransactionError::Connection(db_err) => Err(ServiceError::DatabaseError(db_err)),
                    TransactionError::Transaction(service_err) => Err(service_err),
                }
            }
        }
    }
}

impl CompleteReturnCommand {
    /// Validates that the return exists and is in a valid state for completion
    async fn validate_return_state(
        &self,
        db: &DatabaseTransaction,
    ) -> Result<ReturnEntity, ServiceError> {
        let return_request = Return::find_by_id(self.return_id)
            .one(db)
            .await
            .map_err(|e| {
                let msg = format!("Database error when finding return: {}", e);
                tracing::error!(error = %e, return_id = %self.return_id, "{}", msg);
                ServiceError::DatabaseError(msg)
            })?
            .ok_or_else(|| {
                let msg = format!("Return with ID {} not found", self.return_id);
                tracing::warn!(return_id = %self.return_id, "{}", msg);
                ServiceError::NotFound(msg)
            })?;

        // Check if the return is in a valid state for completion
        let current_status = ReturnStatus::from_str(&return_request.status).map_err(|_| {
            let msg = format!("Invalid return status: {}", &return_request.status);
            tracing::error!(status = %return_request.status, return_id = %self.return_id, "{}", msg);
            ServiceError::ValidationError(msg)
        })?;

        // Validate the state transition
        let valid_previous_states = vec![ReturnStatus::ProcessingRefund, ReturnStatus::Inspecting];

        if !valid_previous_states.contains(&current_status) {
            let msg = format!(
                "Cannot complete return in state {}. Return must be in one of the following states: {:?}",
                current_status,
                valid_previous_states
            );
            tracing::warn!(
                current_status = %current_status,
                return_id = %self.return_id,
                valid_states = ?valid_previous_states,
                "{}", msg
            );
            return Err(ServiceError::InvalidState(msg));
        }

        Ok(return_request)
    }

    /// Updates the return status to Completed
    async fn complete_return(
        &self,
        db: &DatabaseTransaction,
        return_request: &ReturnEntity,
    ) -> Result<ReturnEntity, ServiceError> {
        let now = Utc::now().naive_utc();
        let mut return_active: return_entity::ActiveModel = return_request.clone().into();

        // Update return status
        return_active.status = Set(ReturnStatus::Completed.to_string());
        return_active.updated_at = Set(now);

        // Set additional metadata if provided
        if let Some(metadata) = &self.metadata {
            let current_metadata = return_request
                .metadata
                .clone()
                .unwrap_or_else(|| serde_json::json!({}));

            let mut updated_metadata = match current_metadata {
                serde_json::Value::Object(map) => map,
                _ => serde_json::Map::new(),
            };

            if let serde_json::Value::Object(new_data) = metadata {
                for (key, value) in new_data {
                    updated_metadata.insert(key.clone(), value.clone());
                }
            }

            return_active.metadata = Set(Some(serde_json::Value::Object(updated_metadata)));
        }

        let updated_return = return_active.update(db).await.map_err(|e| {
            let msg = format!("Failed to update return status to Completed: {}", e);
            tracing::error!(error = %e, return_id = %self.return_id, "{}", msg);
            ServiceError::DatabaseError(msg)
        })?;

        tracing::debug!(return_id = %self.return_id, "Return status updated to Completed");
        Ok(updated_return)
    }

    /// Adds a note about the completion
    async fn add_completion_note(
        &self,
        txn: &DatabaseTransaction,
        note_text: &str,
    ) -> Result<(), ServiceError> {
        let note = return_note_entity::ActiveModel {
            id: Set(Uuid::new_v4()),
            return_id: Set(self.return_id),
            content: Set(note_text.to_string()),
            created_by: Set(Some(self.completed_by.clone())),
            created_at: Set(Utc::now().naive_utc()),
            ..Default::default()
        };

        note.insert(txn).await.map_err(|e| {
            error!("Failed to add completion note: {}", e);
            ServiceError::DatabaseError(format!("Failed to add completion note: {}", e))
        })?;

        Ok(())
    }

    /// Creates a history record for the completion
    async fn create_history_record(
        &self,
        txn: &DatabaseTransaction,
        return_request: &ReturnEntity,
    ) -> Result<(), ServiceError> {
        let history = return_history_entity::ActiveModel {
            id: Set(Uuid::new_v4()),
            return_id: Set(self.return_id),
            status_from: Set(return_request.status.clone()),
            status_to: Set(ReturnStatus::Completed.to_string()),
            changed_by: Set(Some(self.completed_by.clone())),
            change_reason: Set(Some("Return completed".to_string())),
            notes: Set(self.notes.clone()),
            created_at: Set(Utc::now()),
            ..Default::default()
        };

        ReturnHistory::insert(history).exec(txn).await.map_err(|e| {
            let msg = format!("Failed to create history record: {}", e);
            tracing::error!(error = %e, return_id = %self.return_id, "{}", msg);
            ServiceError::DatabaseError(msg)
        })?;

        tracing::debug!(return_id = %self.return_id, "Created history record for completion");
        Ok(())
    }

    /// Logs the completion and triggers related events
    async fn log_and_trigger_event(
        &self,
        event_sender: &EventSender,
        result: &CompleteReturnResult,
    ) -> Result<(), ServiceError> {
        tracing::info!(
            return_id = %self.return_id,
            completed_by = ?self.completed_by,
            "Return request successfully completed"
        );

        // Create rich event data
        let event_data = crate::events::EventData::ReturnCompleted {
            return_id: self.return_id,
            completed_at: result.completed_at.clone(),
            completed_by: self.completed_by.clone(),
            metadata: self.metadata.clone(),
        };

        // Send the event with rich data
        event_sender
            .send(crate::events::Event::with_data(event_data))
            .await
            .map_err(|e| {
                let msg = format!("Failed to send ReturnCompleted event: {}", e);
                tracing::error!(error = %e, return_id = %self.return_id, "{}", msg);
                ServiceError::EventError(msg)
            })
    }
}
