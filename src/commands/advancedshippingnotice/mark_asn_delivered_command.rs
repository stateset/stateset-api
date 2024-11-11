use std::sync::Arc;
use sea_orm::*;
use crate::{
    db::DbPool,
    errors::ASNError,
    events::{Event, EventSender},
    models::{
        asn_entity::{self, Entity as ASN},
        asn_note_entity,
        ASNStatus,
    },
};
use serde::{Deserialize, Serialize};
use tracing::{error, info, instrument, warn};
use uuid::Uuid;
use validator::Validate;
use prometheus::{IntCounter, IntCounterVec};
use lazy_static::lazy_static;
use chrono::Utc;


#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct MarkASNDeliveredCommand {
    pub asn_id: Uuid,
    pub version: i32,
    pub delivery_date: DateTime<Utc>,
    pub recipient_name: String,
    #[validate(length(max = 1000))]
    pub delivery_notes: Option<String>,
    pub proof_of_delivery: Option<String>,
}

#[async_trait::async_trait]
impl Command for MarkASNDeliveredCommand {
    type Result = ASNResult;

    async fn execute(
        &self,
        db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ASNError> {
        let db = db_pool.as_ref();

        // Validate ASN is in transit
        let current_asn = ASN::find_by_id(self.asn_id)
            .one(db)
            .await?
            .ok_or(ASNError::NotFound(self.asn_id))?;

        if current_asn.status != ASNStatus::InTransit.to_string() {
            return Err(ASNError::InvalidStatus(self.asn_id));
        }

        // Update ASN status and delivery details
        let updated_asn = db.transaction::<_, asn_entity::Model, ASNError>(|txn| {
            Box::pin(async move {
                let mut asn: asn_entity::ActiveModel = current_asn.into();
                asn.status = Set(ASNStatus::Delivered.to_string());
                asn.version = Set(self.version + 1);
                asn.delivery_date = Set(Some(self.delivery_date.naive_utc()));
                asn.recipient_name = Set(Some(self.recipient_name.clone()));
                asn.delivery_notes = Set(self.delivery_notes.clone());
                asn.proof_of_delivery = Set(self.proof_of_delivery.clone());

                asn.update(txn).await
                    .map_err(|e| ASNError::DatabaseError(e.to_string()))
            })
        }).await?;

        event_sender
            .send(Event::ASNDelivered(
                self.asn_id,
                self.delivery_date,
                self.recipient_name.clone()
            ))
            .await?;

        Ok(ASNResult::from(updated_asn))
    }
}
