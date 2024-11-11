use std::sync::Arc;
use sea_orm::*;
use crate::{
    db::DbPool,
    errors::ASNError,
    events::{Event, EventSender},
    models::{
        asn_entity::{self, Entity as ASN},
        asn_note_entity,
        ASNStatus,
    },
};
use serde::{Deserialize, Serialize};
use tracing::{error, info, instrument, warn};
use uuid::Uuid;
use validator::Validate;
use prometheus::{IntCounter, IntCounterVec};
use lazy_static::lazy_static;
use chrono::Utc;

lazy_static! {
    static ref ASN_CANCELLATIONS: IntCounter = 
        IntCounter::new("asn_cancellations_total", "Total number of ASN cancellations")
            .expect("metric can be created");

    static ref ASN_CANCELLATION_FAILURES: IntCounterVec = 
        IntCounterVec::new(
            "asn_cancellation_failures_total",
            "Total number of failed ASN cancellations",
            &["error_type"]
        ).expect("metric can be created");
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct CancelASNCommand {
    pub asn_id: Uuid,
    #[validate(length(min = 1, max = 500, message = "Reason must be between 1 and 500 characters"))]
    pub reason: String,
    pub version: i32,  // For optimistic locking
    pub notify_supplier: bool, // Whether to notify the supplier about cancellation
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CancelASNResult {
    pub id: Uuid,
    pub status: String,
    pub version: i32,
    pub cancellation_reason: String,
    pub cancellation_timestamp: chrono::DateTime<Utc>,
    pub supplier_notified: bool,
}

#[async_trait::async_trait]
impl Command for CancelASNCommand {
    type Result = CancelASNResult;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(
        &self,
        db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ASNError> {
        self.validate().map_err(|e| {
            ASN_CANCELLATION_FAILURES.with_label_values(&["validation_error"]).inc();
            let msg = format!("Invalid input: {}", e);
            error!("{}", msg);
            ASNError::ValidationError(msg)
        })?;

        let db = db_pool.as_ref();

        // Validate ASN can be cancelled in current state
        self.validate_can_cancel(db).await?;

        let updated_asn = self.cancel_asn_in_db(db).await?;

        self.log_and_trigger_events(&event_sender, &updated_asn).await?;

        ASN_CANCELLATIONS.inc();

        Ok(CancelASNResult {
            id: updated_asn.id,
            status: updated_asn.status,
            version: updated_asn.version,
            cancellation_reason: self.reason.clone(),
            cancellation_timestamp: updated_asn.updated_at.and_utc(),
            supplier_notified: self.notify_supplier,
        })
    }
}

impl CancelASNCommand {
    async fn validate_can_cancel(
        &self,
        db: &DatabaseConnection,
    ) -> Result<(), ASNError> {
        let asn = ASN::find_by_id(self.asn_id)
            .one(db)
            .await
            .map_err(|e| ASNError::DatabaseError(e.to_string()))?
            .ok_or(ASNError::NotFound(self.asn_id))?;

        // Can't cancel if already in transit or delivered
        match asn.status.as_str() {
            "IN_TRANSIT" | "DELIVERED" => {
                ASN_CANCELLATION_FAILURES.with_label_values(&["invalid_status"]).inc();
                Err(ASNError::InvalidStatus(self.asn_id))
            },
            _ => Ok(())
        }
    }

    #[instrument(skip(db))]
    async fn cancel_asn_in_db(
        &self,
        db: &DatabaseConnection,
    ) -> Result<asn_entity::Model, ASNError> {
        db.transaction::<_, asn_entity::Model, ASNError>(|txn| {
            Box::pin(async move {
                let asn = ASN::find_by_id(self.asn_id)
                    .one(txn)
                    .await
                    .map_err(|e| ASNError::DatabaseError(e.to_string()))?
                    .ok_or(ASNError::NotFound(self.asn_id))?;

                if asn.version != self.version {
                    warn!("Concurrent modification detected for ASN {}", self.asn_id);
                    return Err(ASNError::ConcurrentModification(self.asn_id));
                }

                let mut asn: asn_entity::ActiveModel = asn.into();
                asn.status = Set(ASNStatus::Cancelled.to_string());
                asn.version = Set(self.version + 1);
                asn.updated_at = Set(Utc::now().naive_utc());

                let updated_asn = asn.update(txn).await
                    .map_err(|e| ASNError::DatabaseError(e.to_string()))?;

                // Add cancellation note
                let new_note = asn_note_entity::ActiveModel {
                    asn_id: Set(self.asn_id),
                    note_type: Set("CANCELLATION".to_string()),
                    note: Set(self.reason.clone()),
                    created_at: Set(Utc::now().naive_utc()),
                    created_by: Set(None), // Could add user context if available
                    ..Default::default()
                };

                new_note.insert(txn).await
                    .map_err(|e| ASNError::DatabaseError(e.to_string()))?;

                Ok(updated_asn)
            })
        }).await
    }

    async fn log_and_trigger_events(
        &self,
        event_sender: &EventSender,
        updated_asn: &asn_entity::Model,
    ) -> Result<(), ASNError> {
        info!(
            asn_id = %self.asn_id,
            reason = %self.reason,
            notify_supplier = %self.notify_supplier,
            "ASN canceled successfully"
        );

        // Send cancellation event
        event_sender
            .send(Event::ASNCancelled(self.asn_id, self.reason.clone()))
            .await
            .map_err(|e| {
                ASN_CANCELLATION_FAILURES.with_label_values(&["event_error"]).inc();
                let msg = format!("Failed to send event for canceled ASN: {}", e);
                error!("{}", msg);
                ASNError::EventError(msg)
            })?;

        // Send supplier notification event if requested
        if self.notify_supplier {
            event_sender
                .send(Event::ASNCancellationNotificationRequested(
                    self.asn_id,
                    updated_asn.supplier_id,
                    self.reason.clone()
                ))
                .await
                .map_err(|e| {
                    ASN_CANCELLATION_FAILURES.with_label_values(&["notification_error"]).inc();
                    let msg = format!("Failed to send supplier notification event: {}", e);
                    error!("{}", msg);
                    ASNError::EventError(msg)
                })?;
        }

        Ok(())
    }
}

#[derive(thiserror::Error, Debug)]
pub enum ASNError {
    #[error("ASN {0} not found")]
    NotFound(Uuid),
    #[error("Cannot cancel ASN {0} in current status")]
    InvalidStatus(Uuid),
    #[error("Database error: {0}")]
    DatabaseError(String),
    #[error("Event error: {0}")]
    EventError(String),
    #[error("Concurrent modification of ASN {0}")]
    ConcurrentModification(Uuid),
    #[error("Validation error: {0}")]
    ValidationError(String),
}

impl ASNError {
    pub fn error_type(&self) -> &str {
        match self {
            ASNError::NotFound(_) => "not_found",
            ASNError::InvalidStatus(_) => "invalid_status",
            ASNError::DatabaseError(_) => "database_error",
            ASNError::EventError(_) => "event_error",
            ASNError::ConcurrentModification(_) => "concurrent_modification",
            ASNError::ValidationError(_) => "validation_error",
        }
    }
}