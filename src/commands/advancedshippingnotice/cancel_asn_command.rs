use crate::{
    commands::Command,
    db::DbPool,
    errors::{ASNError, ServiceError},
    events::{Event, EventSender},
    models::{
        asn_entity::{self, Entity as ASN},
        asn_note_entity, ASNStatus,
    },
};
use chrono::Utc;
use lazy_static::lazy_static;
use prometheus::{IntCounter, IntCounterVec};
use sea_orm::{DbErr, *};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info, instrument, warn};
use uuid::Uuid;
use validator::Validate;

lazy_static! {
    static ref ASN_CANCELLATIONS: IntCounter = IntCounter::new(
        "asn_cancellations_total",
        "Total number of ASN cancellations"
    )
    .expect("metric can be created");
    static ref ASN_CANCELLATION_FAILURES: IntCounterVec = IntCounterVec::new(
        prometheus::Opts::new(
            "asn_cancellation_failures_total",
            "Total number of failed ASN cancellations"
        ),
        &["error_type"]
    )
    .expect("metric can be created");
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct CancelASNCommand {
    pub asn_id: Uuid,
    #[validate(length(
        min = 1,
        max = 500,
        message = "Reason must be between 1 and 500 characters"
    ))]
    pub reason: String,
    pub version: i32,          // For optimistic locking
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
    ) -> Result<Self::Result, ServiceError> {
        self.validate().map_err(|e| {
            ASN_CANCELLATION_FAILURES
                .with_label_values(&["validation_error"])
                .inc();
            let msg = format!("Invalid input: {}", e);
            error!("{}", msg);
            ServiceError::ValidationError(msg)
        })?;

        let db = db_pool.as_ref();

        // Validate ASN can be cancelled in current state
        self.validate_can_cancel(db).await?;

        let updated_asn = self.cancel_asn_in_db(db).await?;

        self.log_and_trigger_events(&event_sender, &updated_asn)
            .await?;

        ASN_CANCELLATIONS.inc();

        Ok(CancelASNResult {
            id: updated_asn.id,
            status: updated_asn.status.to_string(),
            version: updated_asn.version,
            cancellation_reason: self.reason.clone(),
            cancellation_timestamp: updated_asn.updated_at,
            supplier_notified: self.notify_supplier,
        })
    }
}

impl CancelASNCommand {
    async fn validate_can_cancel(&self, db: &DatabaseConnection) -> Result<(), ASNError> {
        let asn = ASN::find_by_id(self.asn_id)
            .one(db)
            .await
            .map_err(|e| ASNError::DatabaseError(e))?
            .ok_or(ASNError::NotFound(self.asn_id.to_string()))?;

        // Can't cancel if already in transit or delivered
        match asn.status {
            ASNStatus::InTransit | ASNStatus::Delivered => {
                ASN_CANCELLATION_FAILURES
                    .with_label_values(&["invalid_status"])
                    .inc();
                Err(ASNError::InvalidStatus(format!(
                    "Cannot cancel ASN {} - invalid status",
                    self.asn_id
                )))
            }
            _ => Ok(()),
        }
    }

    #[instrument(skip(db))]
    async fn cancel_asn_in_db(
        &self,
        db: &DatabaseConnection,
    ) -> Result<asn_entity::Model, ASNError> {
        let asn_id = self.asn_id;
        let version = self.version;
        let reason = self.reason.clone();

        let result = db
            .transaction::<_, asn_entity::Model, DbErr>(|txn| {
                Box::pin(async move {
                    let asn = ASN::find_by_id(asn_id)
                        .one(txn)
                        .await?
                        .ok_or(DbErr::RecordNotFound(format!("ASN {} not found", asn_id)))?;

                    if asn.version != version {
                        warn!("Concurrent modification detected for ASN {}", asn_id);
                        return Err(DbErr::Custom(
                            "Concurrent modification detected".to_string(),
                        ));
                    }

                    let mut asn: asn_entity::ActiveModel = asn.into();
                    asn.status = Set(ASNStatus::Cancelled);
                    asn.version = Set(version + 1);
                    asn.updated_at = Set(Utc::now());

                    let updated_asn = asn.update(txn).await?;

                    // Add cancellation note
                    let new_note = asn_note_entity::ActiveModel {
                        asn_id: Set(asn_id),
                        note_type: Set(asn_note_entity::ASNNoteType::System),
                        note_text: Set(reason),
                        created_at: Set(Utc::now()),
                        created_by: Set(None), // Could add user context if available
                        ..Default::default()
                    };

                    new_note.insert(txn).await?;

                    Ok(updated_asn)
                })
            })
            .await;

        result.map_err(|e| match e {
            sea_orm::TransactionError::Connection(db_err) => ASNError::DatabaseError(db_err),
            sea_orm::TransactionError::Transaction(db_err) => ASNError::DatabaseError(db_err),
        })
    }

    async fn log_and_trigger_events(
        &self,
        event_sender: &EventSender,
        _updated_asn: &asn_entity::Model,
    ) -> Result<(), ASNError> {
        info!(
            asn_id = %self.asn_id,
            reason = %self.reason,
            notify_supplier = %self.notify_supplier,
            "ASN canceled successfully"
        );

        // Send cancellation event
        event_sender
            .send(Event::ASNCancelled(self.asn_id))
            .await
            .map_err(|e| {
                ASN_CANCELLATION_FAILURES
                    .with_label_values(&["event_error"])
                    .inc();
                let msg = format!("Failed to send event for canceled ASN: {}", e);
                error!("{}", msg);
                ASNError::EventError(msg)
            })?;

        // Send supplier notification event if requested
        if self.notify_supplier {
            event_sender
                .send(Event::ASNCancellationNotificationRequested(self.asn_id))
                .await
                .map_err(|e| {
                    ASN_CANCELLATION_FAILURES
                        .with_label_values(&["notification_error"])
                        .inc();
                    let msg = format!("Failed to send supplier notification event: {}", e);
                    error!("{}", msg);
                    ASNError::EventError(msg)
                })?;
        }

        Ok(())
    }
}
