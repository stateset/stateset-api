use crate::commands::Command;
use crate::{
    db::DbPool,
    entities::{
        warranty::{self, Entity as Warranty},
        warranty_claim::{self, Entity as WarrantyClaim},
    },
    errors::ServiceError,
    events::{Event, EventSender},
};
use async_trait::async_trait;
use chrono::Utc;
use lazy_static::lazy_static;
use prometheus::{Counter, IntCounter};
use sea_orm::{*, Set};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info, instrument};
use uuid::Uuid;
use validator::Validate;

lazy_static! {
    static ref WARRANTY_CLAIMS: IntCounter =
        IntCounter::new("warranty_claims_total", "Total number of warranty claims")
            .expect("metric can be created");
    static ref WARRANTY_CLAIM_FAILURES: IntCounter = IntCounter::new(
        "warranty_claim_failures_total",
        "Total number of failed warranty claims"
    )
    .expect("metric can be created");
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct ClaimWarrantyCommand {
    pub warranty_id: Uuid,
    pub customer_id: Uuid,
    #[validate(length(min = 1, message = "Description cannot be empty"))]
    pub description: String,
    pub evidence: Vec<String>,
    pub contact_email: Option<String>,
    pub contact_phone: Option<String>,
}

#[async_trait]
impl Command for ClaimWarrantyCommand {
    type Result = Uuid;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(
        &self,
        db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        self.validate().map_err(|e| {
            WARRANTY_CLAIM_FAILURES.inc();
            let msg = format!("Invalid input: {}", e);
            error!("{}", msg);
            ServiceError::ValidationError(msg)
        })?;

        let db = db_pool.as_ref();

        // Verify the warranty exists and is active
        let warranty = Warranty::find_by_id(self.warranty_id)
            .one(db)
            .await
            .map_err(|e| {
                WARRANTY_CLAIM_FAILURES.inc();
                let msg = format!("Failed to find warranty: {}", e);
                error!("{}", msg);
                ServiceError::DatabaseError(e)
            })?
            .ok_or_else(|| {
                WARRANTY_CLAIM_FAILURES.inc();
                let msg = format!("Warranty with ID {} not found", self.warranty_id);
                error!("{}", msg);
                ServiceError::NotFound(msg)
            })?;

        // Check if the warranty is active
        if warranty.status != "active" {
            WARRANTY_CLAIM_FAILURES.inc();
            let msg = format!("Warranty is not active (status: {})", warranty.status);
            error!("{}", msg);
            return Err(ServiceError::ValidationError(msg));
        }

        // Check if the customer ID matches
        if warranty.customer_id != self.customer_id {
            WARRANTY_CLAIM_FAILURES.inc();
            let msg = "Customer ID does not match warranty's customer ID".to_string();
            error!("{}", msg);
            return Err(ServiceError::AuthError(msg));
        }

        // Check if the warranty has expired
        let now = Utc::now();
        if now > warranty.end_date {
            WARRANTY_CLAIM_FAILURES.inc();
            let msg = "Warranty has expired".to_string();
            error!("{}", msg);
            return Err(ServiceError::ValidationError(msg));
        }

        // Generate a unique claim number
        let claim_number = format!("WC-{}", uuid::Uuid::new_v4().simple());

        // Create a new warranty claim
        let claim = warranty_claim::ActiveModel {
            id: Set(Uuid::new_v4()),
            warranty_id: Set(self.warranty_id),
            claim_number: Set(claim_number),
            status: Set("submitted".to_string()),
            claim_date: Set(now),
            description: Set(self.description.clone()),
            resolution: Set(None),
            resolved_date: Set(None),
            created_at: Set(now),
            updated_at: Set(None),
        };

        let result = claim.insert(db).await.map_err(|e| {
            WARRANTY_CLAIM_FAILURES.inc();
            let msg = format!("Failed to create warranty claim: {}", e);
            error!("{}", msg);
            ServiceError::DatabaseError(e)
        })?;

        // Send warranty claim event
        event_sender
            .send(Event::WarrantyClaimed(self.warranty_id))
            .await
            .map_err(|e| {
                WARRANTY_CLAIM_FAILURES.inc();
                let msg = format!("Failed to send warranty claim event: {}", e);
                error!("{}", msg);
                ServiceError::EventError(msg)
            })?;

        info!(
            warranty_id = %self.warranty_id,
            claim_id = %result.id,
            customer_id = %self.customer_id,
            "Warranty claim created successfully"
        );

        WARRANTY_CLAIMS.inc();

        Ok(result.id)
    }
}
