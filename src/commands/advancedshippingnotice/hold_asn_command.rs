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
    static ref ASN_HOLDS: IntCounter =
        IntCounter::new("asn_holds_total", "Total number of ASNs placed on hold")
            .expect("metric can be created");
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct HoldASNCommand {
    pub asn_id: Uuid,
    #[validate(length(min = 1, max = 500))]
    pub reason: String,
    pub version: i32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HoldASNResult {
    pub id: Uuid,
    pub status: String,
    pub version: i32,
    pub hold_reason: String,
}

#[async_trait::async_trait]
impl Command for HoldASNCommand {
    type Result = HoldASNResult;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(
        &self,
        db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        self.validate()
            .map_err(|e| ServiceError::ValidationError(e.to_string()))?;

        let db = db_pool.as_ref();

        let updated_asn = self.place_on_hold(db).await?;

        self.log_and_trigger_event(&event_sender).await?;

        ASN_HOLDS.inc();

        Ok(HoldASNResult {
            id: updated_asn.id,
            status: updated_asn.status,
            version: updated_asn.version,
            hold_reason: self.reason.clone(),
        })
    }
}

impl HoldASNCommand {
    async fn place_on_hold(
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

                if asn.status == ASNStatus::Completed.to_string() || asn.status == ASNStatus::Cancelled.to_string() {
                    return Err(ServiceError::ValidationError("Cannot hold completed or cancelled ASN".to_string()));
                }

                let mut asn: asn_entity::ActiveModel = asn.into();
                asn.status = Set(ASNStatus::OnHold.to_string());
                asn.version = Set(self.version + 1);
                asn.updated_at = Set(Utc::now().naive_utc());

                let updated = asn.update(txn).await
                    .map_err(|e| ServiceError::DatabaseError(e.to_string()))?;

                let note = asn_note_entity::ActiveModel {
                    asn_id: Set(self.asn_id),
                    note_type: Set("HOLD".to_string()),
                    note: Set(self.reason.clone()),
                    created_at: Set(Utc::now().naive_utc()),
                    created_by: Set(None),
                    ..Default::default()
                };
                note.insert(txn).await
                    .map_err(|e| ServiceError::DatabaseError(e.to_string()))?;

                Ok(updated)
            })
        }).await
    }

    async fn log_and_trigger_event(&self, event_sender: &EventSender) -> Result<(), ServiceError> {
        info!(asn_id = %self.asn_id, reason = %self.reason, "ASN placed on hold");
        event_sender
            .send(Event::ASNOnHold(self.asn_id))
            .await
            .map_err(|e| ServiceError::EventError(e.to_string()))
    }
}
