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
    static ref WARRANTY_CLAIM_APPROVALS: IntCounter = IntCounter::new(
        "warranty_claim_approvals_total",
        "Total number of warranty claim approvals"
    )
    .expect("metric can be created");
    static ref WARRANTY_CLAIM_APPROVAL_FAILURES: IntCounter = IntCounter::new(
        "warranty_claim_approval_failures_total",
        "Total number of failed warranty claim approvals"
    )
    .expect("metric can be created");
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct ApproveWarrantyClaimCommand {
    pub claim_id: Uuid,
    pub approved_by: Uuid,
    #[validate(length(min = 1, message = "Resolution cannot be empty"))]
    pub resolution: String,
    pub notes: Option<String>,
}

#[async_trait]
impl Command for ApproveWarrantyClaimCommand {
    type Result = warranty_claim::Model;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(
        &self,
        db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        self.validate().map_err(|e| {
            WARRANTY_CLAIM_APPROVAL_FAILURES.inc();
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
                WARRANTY_CLAIM_APPROVAL_FAILURES.inc();
                let msg = format!("Failed to find warranty claim: {}", e);
                error!("{}", msg);
                ServiceError::db_error(e)
            })?
            .ok_or_else(|| {
                WARRANTY_CLAIM_APPROVAL_FAILURES.inc();
                let msg = format!("Warranty claim with ID {} not found", self.claim_id);
                error!("{}", msg);
                ServiceError::NotFound(msg)
            })?;

        // Check if the warranty claim is in a state that can be approved
        if warranty_claim.status != "submitted" {
            WARRANTY_CLAIM_APPROVAL_FAILURES.inc();
            let msg = format!(
                "Warranty claim is not in submitted state (status: {})",
                warranty_claim.status
            );
            error!("{}", msg);
            return Err(ServiceError::ValidationError(msg));
        }

        // Update the warranty claim to approved
        let mut claim_model: warranty_claim::ActiveModel = warranty_claim.clone().into();
        claim_model.status = Set("approved".to_string());
        claim_model.resolution = Set(Some(self.resolution.clone()));
        claim_model.resolved_date = Set(Some(Utc::now()));
        claim_model.updated_at = Set(Some(Utc::now()));

        // Apply the changes to the database
        let updated = claim_model.update(db).await.map_err(|e| {
            WARRANTY_CLAIM_APPROVAL_FAILURES.inc();
            let msg = format!("Failed to update warranty claim: {}", e);
            error!("{}", msg);
            ServiceError::db_error(e)
        })?;

        // Send warranty claim approved event
        event_sender
            .send(Event::WarrantyClaimApproved {
                claim_id: updated.id,
                warranty_id: updated.warranty_id,
                resolution: Some(self.resolution.clone()),
                notes: self.notes.clone(),
            })
            .await
            .map_err(|e| {
                WARRANTY_CLAIM_APPROVAL_FAILURES.inc();
                let msg = format!("Failed to send warranty claim approved event: {}", e);
                error!("{}", msg);
                ServiceError::EventError(msg)
            })?;

        info!(
            claim_id = %self.claim_id,
            approved_by = %self.approved_by,
            "Warranty claim approved successfully"
        );

        WARRANTY_CLAIM_APPROVALS.inc();

        Ok(updated)
    }
}
