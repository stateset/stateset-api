use std::sync::Arc;
use sea_orm::*;
use chrono::Utc;
use crate::{
    db::DbPool,
    errors::ServiceError,
    events::{Event, EventData, EventSender},
    models::{
        return_entity::{self, Entity as Return},
        return_entity::ReturnStatus,
        return_history_entity::{self, Entity as ReturnHistory},
        return_note_entity::{self, Entity as ReturnNote},
    },
    utils::validation,
};
use serde::{Deserialize, Serialize};
use tracing::{debug, error, info, instrument, warn};
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

#[async_trait::async_trait]
impl crate::commands::Command for CompleteReturnCommand {
    type Result = CompleteReturnResult;

    #[instrument(skip(self, db_pool, event_sender), fields(return_id = %self.return_id))]
    async fn execute(
        &self,
        db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        debug!("Executing CompleteReturnCommand");
        let db = db_pool.as_ref();

        // Execute the operation inside a transaction
        let result = db.transaction::<_, Self::Result, ServiceError>(|txn| {
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
                    id: updated_return.id,
                    object: "return".to_string(),
                    completed: true,
                    completed_at: updated_return.updated_at.to_rfc3339(),
                    completed_by: self.completed_by.clone(),
                })
            })
        }).await;

        match result {
            Ok(result) => {
                // Log and trigger event outside the transaction
                self.log_and_trigger_event(&event_sender, &result).await?;
                Ok(result)
            }
            Err(e) => {
                error!("Failed to complete return: {}", e);
                Err(e)
            }
        }
    }
}

impl CompleteReturnCommand {
    /// Validates that the return exists and is in a valid state for completion
    async fn validate_return_state(
        &self,
        db: &DatabaseConnection,
    ) -> Result<return_entity::Model, ServiceError> {
        let return_request = Return::find_by_id(self.return_id)
            .one(db)
            .await
            .map_err(|e| {
                let msg = format!("Database error when finding return: {}", e);
                error!(error = %e, return_id = %self.return_id, "{}", msg);
                ServiceError::DatabaseError(msg)
            })?
            .ok_or_else(|| {
                let msg = format!("Return with ID {} not found", self.return_id);
                warn!(return_id = %self.return_id, "{}", msg);
                ServiceError::NotFound(msg)
            })?;

        // Check if the return is in a valid state for completion
        let current_status = ReturnStatus::from_str(&return_request.status)
            .map_err(|_| {
                let msg = format!("Invalid return status: {}", return_request.status);
                error!(status = %return_request.status, return_id = %self.return_id, "{}", msg);
                ServiceError::ValidationError(msg)
            })?;

        // Validate the state transition
        let valid_previous_states = vec![
            ReturnStatus::ProcessingRefund,
            ReturnStatus::Inspecting,
        ];

        if !valid_previous_states.contains(&current_status) {
            let msg = format!(
                "Cannot complete return in state {}. Return must be in one of the following states: {:?}",
                current_status,
                valid_previous_states
            );
            warn!(
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
        db: &DatabaseConnection,
        return_request: &return_entity::Model,
    ) -> Result<return_entity::Model, ServiceError> {
        let now = Utc::now().naive_utc();
        let mut return_active: return_entity::ActiveModel = return_request.clone().into();
        
        return_active.status = Set(ReturnStatus::Completed.to_string());
        return_active.updated_at = Set(now);
        
        // Set additional metadata if provided
        if let Some(metadata) = &self.metadata {
            let current_metadata = return_request.metadata.clone()
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
            error!(error = %e, return_id = %self.return_id, "{}", msg);
            ServiceError::DatabaseError(msg)
        })?;

        debug!(return_id = %self.return_id, "Return status updated to Completed");
        Ok(updated_return)
    }

    /// Adds a note about the completion
    async fn add_completion_note(
        &self,
        db: &DatabaseConnection,
        note_text: &str,
    ) -> Result<(), ServiceError> {
        let note = return_note_entity::ActiveModel {
            id: Set(Uuid::new_v4()),
            return_id: Set(self.return_id),
            note: Set(note_text.to_string()),
            created_by: Set(self.completed_by.clone()),
            created_at: Set(Utc::now().naive_utc()),
            ..Default::default()
        };

        ReturnNote::insert(note)
            .exec(db)
            .await
            .map_err(|e| {
                let msg = format!("Failed to add completion note: {}", e);
                error!(error = %e, return_id = %self.return_id, "{}", msg);
                ServiceError::DatabaseError(msg)
            })?;

        debug!(return_id = %self.return_id, "Added completion note");
        Ok(())
    }

    /// Creates a history record for the completion
    async fn create_history_record(
        &self,
        db: &DatabaseConnection,
        return_request: &return_entity::Model,
    ) -> Result<(), ServiceError> {
        let history = return_history_entity::ActiveModel {
            id: Set(Uuid::new_v4()),
            return_id: Set(self.return_id),
            action: Set("completed".to_string()),
            from_status: Set(Some(return_request.status.clone())),
            to_status: Set(Some(ReturnStatus::Completed.to_string())),
            actor_id: Set(self.completed_by.clone()),
            actor_type: Set(Some("user".to_string())),
            metadata: Set(self.metadata.clone()),
            created_at: Set(Utc::now().naive_utc()),
            ..Default::default()
        };

        ReturnHistory::insert(history)
            .exec(db)
            .await
            .map_err(|e| {
                let msg = format!("Failed to create history record: {}", e);
                error!(error = %e, return_id = %self.return_id, "{}", msg);
                ServiceError::DatabaseError(msg)
            })?;

        debug!(return_id = %self.return_id, "Created history record for completion");
        Ok(())
    }

    /// Logs the completion and triggers related events
    async fn log_and_trigger_event(
        &self,
        event_sender: &EventSender,
        result: &CompleteReturnResult,
    ) -> Result<(), ServiceError> {
        info!(
            return_id = %self.return_id,
            completed_by = ?self.completed_by,
            "Return request successfully completed"
        );

        // Create rich event data
        let event_data = EventData::ReturnCompleted {
            return_id: self.return_id,
            completed_at: result.completed_at.clone(),
            completed_by: self.completed_by.clone(),
            metadata: self.metadata.clone(),
        };
        
        // Send the event with rich data
        event_sender
            .send(Event::with_data(event_data))
            .await
            .map_err(|e| {
                let msg = format!("Failed to send ReturnCompleted event: {}", e);
                error!(error = %e, return_id = %self.return_id, "{}", msg);
                ServiceError::EventError(msg)
            })
    }
}