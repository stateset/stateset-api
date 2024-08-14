use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info, instrument};
use validator::Validate;

use crate::{
    db::DbPool,
    errors::ServiceError,
    events::{Event, EventSender},
    models::{Promotion, PromotionStatus},
};
use diesel::prelude::*;
use chrono::{DateTime, Utc};

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct DeactivatePromotionCommand {
    pub promotion_id: i32,
}

#[async_trait]
impl Command for DeactivatePromotionCommand {
    type Result = Promotion;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|e| {
            error!("Failed to get database connection: {:?}", e);
            ServiceError::DatabaseError
        })?;

        let updated_promotion = self.deactivate_promotion(&conn)?;

        self.log_and_trigger_event(event_sender, &updated_promotion).await?;

        Ok(updated_promotion)
    }
}

impl DeactivatePromotionCommand {
    fn deactivate_promotion(&self, conn: &PgConnection) -> Result<Promotion, ServiceError> {
        diesel::update(promotions::table.find(self.promotion_id))
            .set(promotions::status.eq(PromotionStatus::Inactive))
            .get_result::<Promotion>(conn)
            .map_err(|e| {
                error!(
                    "Failed to deactivate promotion ID {}: {:?}",
                    self.promotion_id, e
                );
                ServiceError::DatabaseError
            })
    }

    async fn log_and_trigger_event(
        &self,
        event_sender: Arc<EventSender>,
        promotion: &Promotion,
    ) -> Result<(), ServiceError> {
        info!("Promotion deactivated: {}", promotion.id);

        event_sender
            .send(Event::PromotionDeactivated(promotion.id))
            .await
            .map_err(|e| {
                error!(
                    "Failed to send PromotionDeactivated event for promotion ID {}: {:?}",
                    promotion.id, e
                );
                ServiceError::EventError(e.to_string())
            })
    }
}
