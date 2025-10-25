use crate::commands::Command;
use crate::{
    db::DbPool,
    entities::warranty_claim::{self, Entity as WarrantyClaim},
    errors::ServiceError,
    events::{Event, EventSender},
};
use async_trait::async_trait;
use chrono::Utc;
use lazy_static::lazy_static;
use prometheus::IntCounter;
use sea_orm::{Set, *};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info, instrument};
use uuid::Uuid;
use validator::Validate;

lazy_static! {
    static ref WARRANTY_CLAIM_REJECTIONS: IntCounter = IntCounter::new(
        "warranty_claim_rejections_total",
        "Total number of warranty claim rejections"
    )
    .expect("metric can be created");
    static ref WARRANTY_CLAIM_REJECTION_FAILURES: IntCounter = IntCounter::new(
        "warranty_claim_rejection_failures_total",
        "Total number of failed warranty claim rejections"
    )
    .expect("metric can be created");
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct RejectWarrantyClaimCommand {
    pub claim_id: Uuid,
    pub rejected_by: Uuid,
    #[validate(length(min = 1, message = "Reason cannot be empty"))]
    pub reason: String,
    pub notes: Option<String>,
}

#[async_trait]
impl Command for RejectWarrantyClaimCommand {
    type Result = warranty_claim::Model;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(
        &self,
        db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        self.validate().map_err(|e| {
            WARRANTY_CLAIM_REJECTION_FAILURES.inc();
            let msg = format!("Invalid input: {}", e);
            error!("{}", msg);
            ServiceError::ValidationError(msg)
        })?;

        let db = db_pool.as_ref();

        // Verify the warranty claim exists
        let warranty_claim = WarrantyClaim::find_by_id(self.claim_id)
            .one(db)
            .await
            .map_err(|e| {
                WARRANTY_CLAIM_REJECTION_FAILURES.inc();
                let msg = format!("Failed to find warranty claim: {}", e);
                error!("{}", msg);
                ServiceError::db_error(e)
            })?
            .ok_or_else(|| {
                WARRANTY_CLAIM_REJECTION_FAILURES.inc();
                let msg = format!("Warranty claim with ID {} not found", self.claim_id);
                error!("{}", msg);
                ServiceError::NotFound(msg)
            })?;

        // Check if the warranty claim is in a state that can be rejected
        if warranty_claim.status != "submitted" {
            WARRANTY_CLAIM_REJECTION_FAILURES.inc();
            let msg = format!(
                "Warranty claim is not in submitted state (status: {})",
                warranty_claim.status
            );
            error!("{}", msg);
            return Err(ServiceError::ValidationError(msg));
        }

        // Update the warranty claim to rejected
        let mut claim_model: warranty_claim::ActiveModel = warranty_claim.clone().into();
        claim_model.status = Set("rejected".to_string());
        claim_model.resolution = Set(Some(format!("Rejected: {}", self.reason)));
        claim_model.resolved_date = Set(Some(Utc::now()));
        claim_model.updated_at = Set(Some(Utc::now()));

        // Apply the changes to the database
        let updated = claim_model.update(db).await.map_err(|e| {
            WARRANTY_CLAIM_REJECTION_FAILURES.inc();
            let msg = format!("Failed to update warranty claim: {}", e);
            error!("{}", msg);
            ServiceError::db_error(e)
        })?;

        // Send warranty claim rejected event
        event_sender
            .send(Event::WarrantyClaimRejected {
                claim_id: updated.id,
                warranty_id: updated.warranty_id,
                reason: self.reason.clone(),
                notes: self.notes.clone(),
            })
            .await
            .map_err(|e| {
                WARRANTY_CLAIM_REJECTION_FAILURES.inc();
                let msg = format!("Failed to send warranty claim rejected event: {}", e);
                error!("{}", msg);
                ServiceError::EventError(msg)
            })?;

        info!(
            claim_id = %self.claim_id,
            rejected_by = %self.rejected_by,
            reason = %self.reason,
            "Warranty claim rejected successfully"
        );

        WARRANTY_CLAIM_REJECTIONS.inc();

        Ok(updated)
    }
}
