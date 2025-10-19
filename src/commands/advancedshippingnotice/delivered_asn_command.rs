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
use tracing::{info, instrument};
use uuid::Uuid;
use validator::Validate;

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct DeliveredASNCommand {
    pub asn_id: Uuid,
    pub user_id: Uuid,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeliveredASNResult {
    pub id: Uuid,
    pub status: String,
    pub updated_at: DateTime<Utc>,
}

#[async_trait::async_trait]
impl Command for DeliveredASNCommand {
    type Result = DeliveredASNResult;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(
        &self,
        db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        let db = db_pool.as_ref();

        // Find the ASN
        let current_asn = ASN::find_by_id(self.asn_id)
            .one(db)
            .await
            .map_err(|e| ServiceError::db_error(e))?
            .ok_or(ServiceError::NotFound(self.asn_id.to_string()))?;

        // Validate that the ASN can be marked as delivered
        if current_asn.status != ASNStatus::InTransit {
            return Err(ServiceError::ValidationError(format!(
                "ASN {} cannot be marked as delivered from current status",
                self.asn_id
            )));
        }

        // Update ASN status to Delivered
        let updated_asn = db
            .transaction::<_, asn_entity::Model, ServiceError>(|txn| {
                Box::pin(async move {
                    let mut asn: asn_entity::ActiveModel = current_asn.into();
                    asn.status = Set(ASNStatus::Delivered);
                    asn.updated_at = Set(Utc::now());

                    asn.update(txn).await.map_err(|e| ServiceError::db_error(e))
                })
            })
            .await
            .map_err(|err| match err {
                sea_orm::TransactionError::Connection(db_err) => ServiceError::db_error(db_err),
                sea_orm::TransactionError::Transaction(service_err) => service_err,
            })?;

        // Send event
        event_sender
            .send(Event::ASNDelivered(self.asn_id))
            .await
            .map_err(|e| ServiceError::EventError(e.to_string()))?;

        info!(
            "ASN {} marked as delivered by user {}",
            self.asn_id, self.user_id
        );

        Ok(DeliveredASNResult {
            id: updated_asn.id,
            status: updated_asn.status.to_string(),
            updated_at: updated_asn.updated_at,
        })
    }
}
