use crate::{
    commands::Command,
    db::DbPool,
    errors::ServiceError,
    events::{Event, EventSender},
    models::{
        asn_entity::{self, Entity as ASN, ASNStatus},
    },
};
use chrono::{DateTime, Utc};
use sea_orm::{*, Set};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::instrument;
use uuid::Uuid;
use validator::Validate;

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct MarkASNInTransitCommand {
    pub asn_id: Uuid,
    pub version: i32,
    pub carrier_details: CarrierDetails,
    pub departure_time: DateTime<Utc>,
    pub estimated_delivery: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct CarrierDetails {
    #[validate(length(min = 1))]
    pub carrier_name: String,
    #[validate(length(min = 1))]
    pub tracking_number: String,
    pub service_level: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MarkASNInTransitResult {
    pub id: Uuid,
    pub status: String,
    pub version: i32,
    pub carrier_details: CarrierDetails,
    pub departure_time: DateTime<Utc>,
    pub estimated_delivery: DateTime<Utc>,
}

#[async_trait::async_trait]
impl Command for MarkASNInTransitCommand {
    type Result = MarkASNInTransitResult;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(
        &self,
        db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        let db = db_pool.as_ref();

        // Validate ASN status
        let current_asn = ASN::find_by_id(self.asn_id)
            .one(db)
            .await
            .map_err(|e| ServiceError::DatabaseError(e))?
            .ok_or(ServiceError::NotFound(self.asn_id.to_string()))?;

        if current_asn.status != ASNStatus::Submitted {
            return Err(ServiceError::ValidationError(format!("ASN {} cannot be marked as in transit from current status", self.asn_id)));
        }

        // Update ASN with shipping details
        let version = self.version;
        let updated_asn = db
            .transaction::<_, asn_entity::Model, ServiceError>(|txn| {
                Box::pin(async move {
                    let mut asn: asn_entity::ActiveModel = current_asn.into();
                    asn.status = Set(ASNStatus::InTransit);
                    asn.version = Set(version + 1);
                    asn.updated_at = Set(Utc::now());
                    // Note: carrier_details, departure_time, estimated_delivery fields
                    // are not part of the ASN model - they would be stored separately

                    asn.update(txn)
                        .await
                        .map_err(|e| ServiceError::DatabaseError(e))
                })
            })
            .await
            .map_err(|e| ServiceError::DatabaseError(e))?;

        event_sender
            .send(Event::ASNInTransit(self.asn_id))
            .await
            .map_err(|e| ServiceError::EventError(e.to_string()))?;

        Ok(MarkASNInTransitResult {
            id: updated_asn.id,
            status: updated_asn.status.to_string(),
            version: updated_asn.version,
            carrier_details: self.carrier_details.clone(),
            departure_time: self.departure_time,
            estimated_delivery: self.estimated_delivery,
        })
    }
}
