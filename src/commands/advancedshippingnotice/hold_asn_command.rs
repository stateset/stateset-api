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
        let asn_id = self.asn_id;
        let version = self.version;
        let reason = self.reason.clone();
        
        let result = db.transaction::<_, asn_entity::Model, DbErr>(|txn| {
            Box::pin(async move {
                let asn = ASN::find_by_id(asn_id)
                    .one(txn)
                    .await
                    ?
                    .ok_or_else(|| {
                        DbErr::RecordNotFound(format!("ASN {} not found", asn_id))
                    })?;

                if asn.version != version {
                    return Err(DbErr::Custom("Concurrent modification".to_string()));
                }

                if asn.status == ASNStatus::Completed
                    || asn.status == ASNStatus::Cancelled
                {
                    return Err(DbErr::Custom("Cannot hold completed or cancelled ASN".to_string()));
                }

                let mut asn: asn_entity::ActiveModel = asn.into();
                asn.status = Set(ASNStatus::OnHold);
                asn.version = Set(version + 1);
                asn.updated_at = Set(Utc::now());

                let updated = asn
                    .update(txn)
                    .await
                    ?;

                let note = asn_note_entity::ActiveModel {
                    asn_id: Set(asn_id),
                    note_type: Set(asn_note_entity::ASNNoteType::System),
                    note_text: Set(reason),
                    created_at: Set(Utc::now()),
                    created_by: Set(None),
                    ..Default::default()
                };
                note.insert(txn)
                    .await
                    ?;

                Ok(updated)
            })
        })
        .await;
        
        result.map_err(|e| match e {
            sea_orm::TransactionError::Connection(db_err) => ServiceError::DatabaseError(db_err),
            sea_orm::TransactionError::Transaction(db_err) => ServiceError::DatabaseError(db_err),
        })
    }

    async fn log_and_trigger_event(&self, event_sender: &EventSender) -> Result<(), ServiceError> {
        info!(asn_id = %self.asn_id, reason = %self.reason, "ASN placed on hold");
        event_sender
            .send(Event::ASNOnHold(self.asn_id))
            .await
            .map_err(|e| ServiceError::EventError(e.to_string()))
    }
}
