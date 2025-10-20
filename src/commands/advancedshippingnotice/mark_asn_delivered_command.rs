use crate::{
    commands::Command,
    db::DbPool,
    errors::ServiceError,
    events::{Event, EventSender},
    models::asn_entity::{self, ASNStatus, Entity as ASN},
};
use chrono::{DateTime, Utc};
use sea_orm::{ActiveModelTrait, EntityTrait, Set, TransactionTrait};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::instrument;
use uuid::Uuid;
use validator::Validate;

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

#[derive(Debug, Serialize, Deserialize)]
pub struct MarkASNDeliveredResult {
    pub id: Uuid,
    pub status: String,
    pub version: i32,
    pub delivery_date: DateTime<Utc>,
    pub recipient_name: String,
    pub delivery_notes: Option<String>,
    pub proof_of_delivery: Option<String>,
}

#[async_trait::async_trait]
impl Command for MarkASNDeliveredCommand {
    type Result = MarkASNDeliveredResult;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(
        &self,
        db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        let db = db_pool.as_ref();

        // Validate ASN is in transit
        let current_asn = ASN::find_by_id(self.asn_id)
            .one(db)
            .await
            .map_err(|e| ServiceError::db_error(e))?
            .ok_or(ServiceError::NotFound(self.asn_id.to_string()))?;

        if current_asn.status != ASNStatus::InTransit {
            return Err(ServiceError::ValidationError(format!(
                "ASN {} cannot be marked as delivered from current status",
                self.asn_id
            )));
        }

        // Update ASN status and delivery details
        let version = self.version;
        let updated_asn = db
            .transaction::<_, asn_entity::Model, ServiceError>(|txn| {
                Box::pin(async move {
                    let mut asn: asn_entity::ActiveModel = current_asn.into();
                    asn.status = Set(ASNStatus::Delivered);
                    asn.version = Set(version + 1);
                    asn.updated_at = Set(Utc::now());
                    // Note: delivery_date, recipient_name, delivery_notes, proof_of_delivery fields
                    // are not part of the ASN model - they would be stored separately

                    asn.update(txn).await.map_err(|e| ServiceError::db_error(e))
                })
            })
            .await
            .map_err(|err| match err {
                sea_orm::TransactionError::Connection(db_err) => ServiceError::db_error(db_err),
                sea_orm::TransactionError::Transaction(service_err) => service_err,
            })?;

        event_sender
            .send(Event::ASNDelivered(self.asn_id))
            .await
            .map_err(|e| ServiceError::EventError(e.to_string()))?;

        Ok(MarkASNDeliveredResult {
            id: updated_asn.id,
            status: updated_asn.status.to_string(),
            version: updated_asn.version,
            delivery_date: self.delivery_date,
            recipient_name: self.recipient_name.clone(),
            delivery_notes: self.delivery_notes.clone(),
            proof_of_delivery: self.proof_of_delivery.clone(),
        })
    }
}
