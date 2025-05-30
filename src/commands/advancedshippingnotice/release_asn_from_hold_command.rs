use std::sync::Arc;
use sea_orm::*;
use crate::{
    db::DbPool,
    errors::ServiceError,
    events::{Event, EventSender},
    models::{
        asn_entity::{self, Entity as ASN, ASNStatus},
        asn_note_entity,
    },
    commands::Command,
};
use serde::{Deserialize, Serialize};
use tracing::{error, info, instrument};
use uuid::Uuid;
use validator::Validate;
use chrono::Utc;
use lazy_static::lazy_static;
use prometheus::IntCounter;

lazy_static! {
    static ref ASN_RELEASES: IntCounter =
        IntCounter::new("asn_release_total", "Total number of ASNs released from hold")
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
        db.transaction::<_, asn_entity::Model, ServiceError>(|txn| {
            Box::pin(async move {
                let asn = ASN::find_by_id(self.asn_id)
                    .one(txn)
                    .await
                    .map_err(|e| ServiceError::DatabaseError(e.to_string()))?
                    .ok_or_else(|| ServiceError::NotFoundError(format!("ASN {} not found", self.asn_id)))?;

                if asn.version != self.version {
                    return Err(ServiceError::ValidationError("Concurrent modification".to_string()));
                }

                if asn.status != ASNStatus::OnHold.to_string() {
                    return Err(ServiceError::ValidationError("ASN must be on hold to release".to_string()));
                }

                let mut asn: asn_entity::ActiveModel = asn.into();
                asn.status = Set(self.target_status.to_string());
                asn.version = Set(self.version + 1);
                asn.updated_at = Set(Utc::now().naive_utc());

                let updated = asn.update(txn).await
                    .map_err(|e| ServiceError::DatabaseError(e.to_string()))?;

                if let Some(note) = &self.notes {
                    let note_model = asn_note_entity::ActiveModel {
                        asn_id: Set(self.asn_id),
                        note_type: Set("RELEASE".to_string()),
                        note: Set(note.clone()),
                        created_at: Set(Utc::now().naive_utc()),
                        created_by: Set(None),
                        ..Default::default()
                    };
                    note_model.insert(txn).await
                        .map_err(|e| ServiceError::DatabaseError(e.to_string()))?;
                }

                Ok(updated)
            })
        }).await
    }

    async fn log_and_trigger_event(&self, event_sender: &EventSender) -> Result<(), ServiceError> {
        info!(asn_id = %self.asn_id, "ASN released from hold");
        event_sender
            .send(Event::ASNReleasedFromHold(self.asn_id))
            .await
            .map_err(|e| ServiceError::EventError(e.to_string()))
    }
}
