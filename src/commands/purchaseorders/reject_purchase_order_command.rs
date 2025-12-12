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
use tracing::{error, info, instrument};
use uuid::Uuid;
use validator::Validate;

lazy_static! {
    static ref PO_REJECTIONS: IntCounter = IntCounter::new(
        "purchase_order_rejections_total",
        "Total number of purchase orders rejected"
    )
    .expect("metric can be created");
    static ref PO_REJECTION_FAILURES: IntCounterVec = IntCounterVec::new(
        prometheus::Opts::new(
            "purchase_order_rejection_failures_total",
            "Total number of failed purchase order rejections"
        ),
        &["error_type"]
    )
    .expect("metric can be created");
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct RejectPurchaseOrderCommand {
    pub id: Uuid,
    pub rejector_id: Uuid,
    #[validate(length(
        min = 1,
        max = 500,
        message = "Rejection reason is required and must be at most 500 characters"
    ))]
    pub reason: String,
    #[validate(length(max = 1000))]
    pub notes: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RejectPurchaseOrderResult {
    pub id: Uuid,
    pub status: String,
    pub rejected_at: chrono::DateTime<Utc>,
    pub rejection_reason: String,
}

#[async_trait::async_trait]
impl Command for RejectPurchaseOrderCommand {
    type Result = RejectPurchaseOrderResult;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(
        &self,
        db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        self.validate().map_err(|e| {
            PO_REJECTION_FAILURES
                .with_label_values(&["validation_error"])
                .inc();
            let msg = format!("Invalid input: {}", e);
            error!("{}", msg);
            ServiceError::ValidationError(msg)
        })?;

        let db = db_pool.as_ref();

        // Validate PO can be rejected
        self.validate_can_reject(db).await?;

        let updated_po = self.reject_purchase_order(db).await?;

        self.log_and_trigger_event(&event_sender, &updated_po)
            .await?;

        PO_REJECTIONS.inc();

        Ok(RejectPurchaseOrderResult {
            id: updated_po.id,
            status: updated_po.status.to_string(),
            rejected_at: updated_po.updated_at,
            rejection_reason: self.reason.clone(),
        })
    }
}

impl RejectPurchaseOrderCommand {
    async fn validate_can_reject(&self, db: &DatabaseConnection) -> Result<(), ServiceError> {
        let po = PurchaseOrder::find_by_id(self.id)
            .one(db)
            .await
            .map_err(|e| {
                PO_REJECTION_FAILURES.with_label_values(&["db_error"]).inc();
                ServiceError::db_error(e)
            })?
            .ok_or_else(|| {
                PO_REJECTION_FAILURES
                    .with_label_values(&["not_found"])
                    .inc();
                ServiceError::NotFound(format!("Purchase order {} not found", self.id))
            })?;

        // Can only reject from Submitted status
        if po.status != PurchaseOrderStatus::Submitted {
            PO_REJECTION_FAILURES
                .with_label_values(&["invalid_status"])
                .inc();
            return Err(ServiceError::InvalidOperation(format!(
                "Cannot reject purchase order in {} status. Must be in Submitted status.",
                po.status.to_string()
            )));
        }

        Ok(())
    }

    async fn reject_purchase_order(
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
        po.status = Set(PurchaseOrderStatus::Rejected);
        po.updated_at = Set(Utc::now());

        // Add rejection reason to notes
        let existing_notes = match po.notes.clone() {
            sea_orm::ActiveValue::Set(opt) | sea_orm::ActiveValue::Unchanged(opt) => {
                opt.unwrap_or_default()
            }
            sea_orm::ActiveValue::NotSet => String::new(),
        };
        let rejection_note = format!(
            "[REJECTED by {}] Reason: {}{}",
            self.rejector_id,
            self.reason,
            self.notes
                .as_ref()
                .map(|n| format!("\nNotes: {}", n))
                .unwrap_or_default()
        );
        let new_notes = if existing_notes.is_empty() {
            rejection_note
        } else {
            format!("{}\n{}", existing_notes, rejection_note)
        };
        po.notes = Set(Some(new_notes));

        po.update(db).await.map_err(|e| {
            PO_REJECTION_FAILURES.with_label_values(&["db_error"]).inc();
            let msg = format!("Failed to reject purchase order {}: {}", self.id, e);
            error!("{}", msg);
            ServiceError::db_error(e)
        })
    }

    async fn log_and_trigger_event(
        &self,
        event_sender: &EventSender,
        _updated_po: &purchase_order_entity::Model,
    ) -> Result<(), ServiceError> {
        info!(
            purchase_order_id = %self.id,
            rejector_id = %self.rejector_id,
            reason = %self.reason,
            "Purchase order rejected"
        );

        event_sender
            .send(Event::Generic {
                message: format!("Purchase order {} rejected", self.id),
                timestamp: Utc::now(),
                metadata: serde_json::json!({
                    "purchase_order_id": self.id,
                    "rejector_id": self.rejector_id,
                    "reason": self.reason,
                    "event_type": "purchase_order_rejected"
                }),
            })
            .await
            .map_err(|e| {
                PO_REJECTION_FAILURES
                    .with_label_values(&["event_error"])
                    .inc();
                let msg = format!("Failed to send event for rejected purchase order: {}", e);
                error!("{}", msg);
                ServiceError::EventError(msg)
            })
    }
}
