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
pub struct MarkASNInTransitCommand {
    pub asn_id: Uuid,
    pub version: i32,
    pub carrier_details: CarrierDetails,
    pub departure_time: DateTime<Utc>,
    pub estimated_delivery: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct CarrierDetails {
    #[validate(length(min = 1))]
    pub carrier_name: String,
    #[validate(length(min = 1))]
    pub tracking_number: String,
    pub service_level: Option<String>,
}

#[async_trait::async_trait]
impl Command for MarkASNInTransitCommand {
    type Result = ASNResult;

    async fn execute(
        &self,
        db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ASNError> {
        let db = db_pool.as_ref();
        
        // Validate ASN status
        let current_asn = ASN::find_by_id(self.asn_id)
            .one(db)
            .await?
            .ok_or(ASNError::NotFound(self.asn_id))?;

        if current_asn.status != ASNStatus::Submitted.to_string() {
            return Err(ASNError::InvalidStatus(self.asn_id));
        }

        // Update ASN with shipping details
        let updated_asn = db.transaction::<_, asn_entity::Model, ASNError>(|txn| {
            Box::pin(async move {
                let mut asn: asn_entity::ActiveModel = current_asn.into();
                asn.status = Set(ASNStatus::InTransit.to_string());
                asn.version = Set(self.version + 1);
                asn.carrier_details = Set(serde_json::to_value(&self.carrier_details)?);
                asn.departure_time = Set(Some(self.departure_time.naive_utc()));
                asn.estimated_delivery = Set(Some(self.estimated_delivery.naive_utc()));
                
                asn.update(txn).await
                    .map_err(|e| ASNError::DatabaseError(e.to_string()))
            })
        }).await?;

        event_sender
            .send(Event::ASNInTransit(
                self.asn_id,
                self.carrier_details.clone(),
                self.estimated_delivery
            ))
            .await?;

        Ok(ASNResult::from(updated_asn))
    }
}