use crate::{
    commands::Command,
    db::DbPool,
    errors::ServiceError,
    events::{Event, EventSender},
    models::{
        asn_entity::{self, ASNStatus, Entity as ASN},
        asn_note_entity,
    },
};
use chrono::Utc;
use lazy_static::lazy_static;
use prometheus::IntCounter;
use sea_orm::{*, Set};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info, instrument};
use uuid::Uuid;
use validator::Validate;

lazy_static! {
    static ref ASN_RELEASES: IntCounter = IntCounter::new(
        "asn_release_total",
        "Total number of ASNs released from hold"
    )
    .expect("metric can be created");
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct ReleaseASNFromHoldCommand {
    pub asn_id: Uuid,
    pub version: i32,
    pub target_status: ASNStatus,
    #[validate(length(min = 1, max = 500))]
    pub notes: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ReleaseASNFromHoldResult {
    pub id: Uuid,
    pub status: String,
    pub version: i32,
}

#[async_trait::async_trait]
impl Command for ReleaseASNFromHoldCommand {
    type Result = ReleaseASNFromHoldResult;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(
        &self,
        db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        self.validate()
            .map_err(|e| ServiceError::ValidationError(e.to_string()))?;

        let db = db_pool.as_ref();

        let updated_asn = self.release_from_hold(db).await?;

        self.log_and_trigger_event(&event_sender).await?;

        ASN_RELEASES.inc();

        Ok(ReleaseASNFromHoldResult {
            id: updated_asn.id,
            status: updated_asn.status,
            version: updated_asn.version,
        })
    }
}

impl ReleaseASNFromHoldCommand {
    async fn release_from_hold(
        &self,
        db: &DatabaseConnection,
    ) -> Result<asn_entity::Model, ServiceError> {
        let asn = ASN::find_by_id(self.asn_id)
            .one(db)
            .await
            .map_err(|e| ServiceError::DatabaseError(e))?
            .ok_or_else(|| {
                ServiceError::NotFound(format!("ASN {} not found", self.asn_id))
            })?;

        if asn.version != self.version {
            return Err(ServiceError::InvalidOperation("Concurrent modification".to_string()));
        }

        if asn.status != ASNStatus::OnHold {
            return Err(ServiceError::InvalidOperation("ASN must be on hold to release".to_string()));
        }

        let mut asn: asn_entity::ActiveModel = asn.into();
        asn.status = Set(self.target_status.clone());
        asn.version = Set(self.version + 1);
        asn.updated_at = Set(Utc::now());

        let updated = asn
            .update(db)
            .await
            .map_err(|e| ServiceError::DatabaseError(e))?;

        if let Some(note) = &self.notes {
            let note_model = asn_note_entity::ActiveModel {
                asn_id: Set(self.asn_id),
                note_type: Set(asn_note_entity::ASNNoteType::System),
                note_text: Set(note.clone()),
                created_at: Set(Utc::now()),
                created_by: Set(None),
                ..Default::default()
            };
            note_model
                .insert(db)
                .await
                .map_err(|e| ServiceError::DatabaseError(e))?;
        }

        Ok(updated)
    }

    async fn log_and_trigger_event(&self, event_sender: &EventSender) -> Result<(), ServiceError> {
        info!(asn_id = %self.asn_id, "ASN released from hold");
        event_sender
            .send(Event::ASNReleasedFromHold(self.asn_id))
            .await
            .map_err(|e| ServiceError::EventError(e.to_string()))
    }
}
