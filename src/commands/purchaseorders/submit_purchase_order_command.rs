use crate::{
    commands::Command,
    db::DbPool,
    errors::ServiceError,
    events::{Event, EventSender},
    models::purchase_order_entity::{self, Entity as PurchaseOrder, PurchaseOrderStatus},
};
use chrono::Utc;
use lazy_static::lazy_static;
use prometheus::{IntCounter, IntCounterVec};
use sea_orm::{Set, *};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info, instrument, warn};
use uuid::Uuid;
use validator::Validate;

lazy_static! {
    static ref PO_SUBMISSIONS: IntCounter = IntCounter::new(
        "purchase_order_submissions_total",
        "Total number of purchase orders submitted"
    )
    .expect("metric can be created");
    static ref PO_SUBMISSION_FAILURES: IntCounterVec = IntCounterVec::new(
        prometheus::Opts::new(
            "purchase_order_submission_failures_total",
            "Total number of failed purchase order submissions"
        ),
        &["error_type"]
    )
    .expect("metric can be created");
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct SubmitPurchaseOrderCommand {
    pub id: Uuid,
    #[validate(length(max = 1000))]
    pub notes: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SubmitPurchaseOrderResult {
    pub id: Uuid,
    pub status: String,
    pub submitted_at: chrono::DateTime<Utc>,
}

#[async_trait::async_trait]
impl Command for SubmitPurchaseOrderCommand {
    type Result = SubmitPurchaseOrderResult;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(
        &self,
        db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        self.validate().map_err(|e| {
            PO_SUBMISSION_FAILURES
                .with_label_values(&["validation_error"])
                .inc();
            let msg = format!("Invalid input: {}", e);
            error!("{}", msg);
            ServiceError::ValidationError(msg)
        })?;

        let db = db_pool.as_ref();

        // Validate PO can be submitted
        self.validate_can_submit(db).await?;

        let updated_po = self.submit_purchase_order(db).await?;

        self.log_and_trigger_event(&event_sender, &updated_po)
            .await?;

        PO_SUBMISSIONS.inc();

        Ok(SubmitPurchaseOrderResult {
            id: updated_po.id,
            status: updated_po.status.to_string(),
            submitted_at: updated_po.updated_at,
        })
    }
}

impl SubmitPurchaseOrderCommand {
    async fn validate_can_submit(&self, db: &DatabaseConnection) -> Result<(), ServiceError> {
        let po = PurchaseOrder::find_by_id(self.id)
            .one(db)
            .await
            .map_err(|e| {
                PO_SUBMISSION_FAILURES
                    .with_label_values(&["db_error"])
                    .inc();
                ServiceError::db_error(e)
            })?
            .ok_or_else(|| {
                PO_SUBMISSION_FAILURES
                    .with_label_values(&["not_found"])
                    .inc();
                ServiceError::NotFound(format!("Purchase order {} not found", self.id))
            })?;

        // Can only submit from Draft status
        if po.status != PurchaseOrderStatus::Draft {
            PO_SUBMISSION_FAILURES
                .with_label_values(&["invalid_status"])
                .inc();
            return Err(ServiceError::InvalidOperation(format!(
                "Cannot submit purchase order in {} status. Must be in Draft status.",
                po.status.to_string()
            )));
        }

        Ok(())
    }

    async fn submit_purchase_order(
        &self,
        db: &DatabaseConnection,
    ) -> Result<purchase_order_entity::Model, ServiceError> {
        let po = PurchaseOrder::find_by_id(self.id)
            .one(db)
            .await
            .map_err(|e| ServiceError::db_error(e))?
            .ok_or_else(|| {
                ServiceError::NotFound(format!("Purchase order {} not found", self.id))
            })?;

        let mut po: purchase_order_entity::ActiveModel = po.into();
        po.status = Set(PurchaseOrderStatus::Submitted);
        po.updated_at = Set(Utc::now());

        if let Some(ref notes) = self.notes {
            // Append submission notes to existing notes
            let existing_notes = match po.notes.clone() {
                sea_orm::ActiveValue::Set(opt) | sea_orm::ActiveValue::Unchanged(opt) => {
                    opt.unwrap_or_default()
                }
                sea_orm::ActiveValue::NotSet => String::new(),
            };
            let new_notes = if existing_notes.is_empty() {
                format!("[SUBMITTED] {}", notes)
            } else {
                format!("{}\n[SUBMITTED] {}", existing_notes, notes)
            };
            po.notes = Set(Some(new_notes));
        }

        po.update(db).await.map_err(|e| {
            PO_SUBMISSION_FAILURES
                .with_label_values(&["db_error"])
                .inc();
            let msg = format!("Failed to submit purchase order {}: {}", self.id, e);
            error!("{}", msg);
            ServiceError::db_error(e)
        })
    }

    async fn log_and_trigger_event(
        &self,
        event_sender: &EventSender,
        updated_po: &purchase_order_entity::Model,
    ) -> Result<(), ServiceError> {
        info!(
            purchase_order_id = %self.id,
            "Purchase order submitted successfully"
        );

        // Use a generic event since PurchaseOrderSubmitted may not exist
        event_sender
            .send(Event::Generic {
                message: format!("Purchase order {} submitted", self.id),
                timestamp: Utc::now(),
                metadata: serde_json::json!({
                    "purchase_order_id": self.id,
                    "event_type": "purchase_order_submitted"
                }),
            })
            .await
            .map_err(|e| {
                PO_SUBMISSION_FAILURES
                    .with_label_values(&["event_error"])
                    .inc();
                let msg = format!("Failed to send event for submitted purchase order: {}", e);
                error!("{}", msg);
                ServiceError::EventError(msg)
            })
    }
}
