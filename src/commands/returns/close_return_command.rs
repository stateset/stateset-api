use crate::{
    commands::Command,
    db::DbPool,
    errors::ServiceError,
    events::{Event, EventData, EventSender},
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

        // Execute the operation inside a transaction
        let result = db
            .transaction::<_, Self::Result, ServiceError>(|txn| {
                Box::pin(async move {
                    // Check if return exists and can be closed
                    let return_request = self.validate_return_state(txn).await?;

                    // Update return status to closed
                    let updated_return = self.close_return(txn, &return_request).await?;

                    // Add closure note if provided
                    if let Some(note) = &self.notes {
                        self.add_closure_note(txn, note).await?;
                    }

                    // Create history record
                    self.create_history_record(txn, &return_request).await?;

                    Ok(CloseReturnResult {
                        id: Uuid::parse_str(&updated_return.id).unwrap_or_else(|_| Uuid::new_v4()),
                        object: "return".to_string(),
                        closed: true,
                        closed_at: updated_return.updated_at.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string(),
                        closed_by: self.closed_by.clone(),
                        reason: self.reason.clone(),
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
    /// Validates that the return exists and is in a valid state for closure
    async fn validate_return_state(
        &self,
        db: &DatabaseTransaction,
    ) -> Result<ReturnEntity, ServiceError> {
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

        // Check if the return is in a valid state for closure
        let current_status = ReturnStatus::from_str(&return_request.status).map_err(|_| {
            let msg = format!("Invalid return status: {}", return_request.status);
            error!(status = %return_request.status, return_id = %self.return_id, "{}", msg);
            ServiceError::ValidationError(msg)
        })?;

        // Validate the state transition
        // Returns can be closed from almost any state except already Closed or Cancelled
        let invalid_states = vec![ReturnStatus::Closed, ReturnStatus::Cancelled];

        if invalid_states.contains(&current_status) {
            let msg = format!(
                "Cannot close return in state {}. Return is already in a terminal state.",
                current_status,
            );
            warn!(
                current_status = %current_status,
                return_id = %self.return_id,
                "{}", msg
            );
            return Err(ServiceError::InvalidState(msg));
        }

        Ok(return_request)
    }

    /// Updates the return status to Closed
    async fn close_return(
        &self,
        db: &DatabaseTransaction,
        return_request: &ReturnEntity,
    ) -> Result<ReturnEntity, ServiceError> {
        let now = Utc::now().naive_utc();
        let mut return_active: ReturnEntity::ActiveModel = return_request.clone().into();

        return_active.status = Set(ReturnStatus::Closed.to_string());
        return_active.updated_at = Set(now);

        // Update metadata with close reason if provided
        let mut updated_metadata = return_request
            .metadata
            .clone()
            .unwrap_or_else(|| serde_json::json!({}));

        if let Some(reason) = &self.reason {
            if let serde_json::Value::Object(ref mut map) = updated_metadata {
                map.insert(
                    "close_reason".to_string(),
                    serde_json::Value::String(reason.clone()),
                );
                map.insert(
                    "closed_at".to_string(),
                    serde_json::Value::String(now.to_string()),
                );
                if let Some(closed_by) = &self.closed_by {
                    map.insert(
                        "closed_by".to_string(),
                        serde_json::Value::String(closed_by.clone()),
                    );
                }
            }
        }

        // Merge additional metadata if provided
        if let Some(metadata) = &self.metadata {
            if let (
                serde_json::Value::Object(ref mut map),
                serde_json::Value::Object(ref new_data),
            ) = (&mut updated_metadata, metadata)
            {
                for (key, value) in new_data {
                    map.insert(key.clone(), value.clone());
                }
            }
        }

        return_active.metadata = Set(Some(updated_metadata));

        let updated_return = return_active.update(db).await.map_err(|e| {
            let msg = format!("Failed to update return status to Closed: {}", e);
            error!(error = %e, return_id = %self.return_id, "{}", msg);
            ServiceError::DatabaseError(msg)
        })?;

        debug!(return_id = %self.return_id, "Return status updated to Closed");
        Ok(updated_return)
    }

    /// Adds a note about the closure
    async fn add_closure_note(
        &self,
        db: &DatabaseTransaction,
        note_text: &str,
    ) -> Result<(), ServiceError> {
        let note_content = if let Some(reason) = &self.reason {
            format!("Return closed: {}. Note: {}", reason, note_text)
        } else {
            format!("Return closed. Note: {}", note_text)
        };

        let note = ReturnNote::ActiveModel {
            id: Set(Uuid::new_v4()),
            return_id: Set(self.return_id),
            note: Set(note_content),
            created_by: Set(self.closed_by.clone()),
            created_at: Set(Utc::now().naive_utc()),
            ..Default::default()
        };

        ReturnNote::insert(note).exec(db).await.map_err(|e| {
            let msg = format!("Failed to add closure note: {}", e);
            error!(error = %e, return_id = %self.return_id, "{}", msg);
            ServiceError::DatabaseError(msg)
        })?;

        debug!(return_id = %self.return_id, "Added closure note");
        Ok(())
    }

    /// Creates a history record for the closure
    async fn create_history_record(
        &self,
        db: &DatabaseTransaction,
        return_request: &ReturnEntity,
    ) -> Result<(), ServiceError> {
        // Prepare metadata for history record
        let mut history_metadata = serde_json::Map::new();

        if let Some(reason) = &self.reason {
            history_metadata.insert(
                "reason".to_string(),
                serde_json::Value::String(reason.clone()),
            );
        }

        // Add custom metadata if provided
        if let Some(serde_json::Value::Object(custom_metadata)) = &self.metadata {
            for (key, value) in custom_metadata {
                history_metadata.insert(key.clone(), value.clone());
            }
        }

        let metadata_value = if history_metadata.is_empty() {
            None
        } else {
            Some(serde_json::Value::Object(history_metadata))
        };

        let history = ReturnHistory::ActiveModel {
            id: Set(Uuid::new_v4()),
            return_id: Set(self.return_id),
            action: Set("closed".to_string()),
            from_status: Set(Some(return_request.status.clone())),
            to_status: Set(Some(ReturnStatus::Closed.to_string())),
            actor_id: Set(self.closed_by.clone()),
            actor_type: Set(Some("user".to_string())),
            metadata: Set(metadata_value),
            created_at: Set(Utc::now().naive_utc()),
            ..Default::default()
        };

        ReturnHistory::insert(history).exec(db).await.map_err(|e| {
            let msg = format!("Failed to create history record: {}", e);
            error!(error = %e, return_id = %self.return_id, "{}", msg);
            ServiceError::DatabaseError(msg)
        })?;

        debug!(return_id = %self.return_id, "Created history record for closure");
        Ok(())
    }

    /// Logs the closure and triggers related events
    async fn log_and_trigger_event(
        &self,
        event_sender: &EventSender,
        result: &CloseReturnResult,
    ) -> Result<(), ServiceError> {
        info!(
            return_id = %self.return_id,
            closed_by = ?self.closed_by,
            reason = ?self.reason,
            "Return request successfully closed"
        );

        // Create rich event data
        let event_data = EventData::ReturnClosed {
            return_id: self.return_id,
            closed_at: result.closed_at.clone(),
            closed_by: self.closed_by.clone(),
            reason: self.reason.clone(),
            metadata: self.metadata.clone(),
        };

        // Send the event with rich data
        event_sender
            .send(Event::with_data(event_data))
            .await
            .map_err(|e| {
                let msg = format!("Failed to send ReturnClosed event: {}", e);
                error!(error = %e, return_id = %self.return_id, "{}", msg);
                ServiceError::EventError(msg)
            })
    }
}
