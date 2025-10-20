use uuid::Uuid;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info, instrument};
use validator::Validate;

use crate::{
    commands::Command,
    db::DbPool,
    errors::ServiceError,
    events::{Event, EventSender},
    models::{promotion_entity, PromotionStatus},
};
use sea_orm::{ActiveModelTrait, ActiveValue::Set, DatabaseConnection, EntityTrait};

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct DeactivatePromotionCommand {
    pub promotion_id: Uuid,
}

#[async_trait::async_trait]
impl Command for DeactivatePromotionCommand {
    type Result = promotion_entity::Model;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(
        &self,
        db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        let db = db_pool.clone();

        let updated_promotion = self.deactivate_promotion(&db).await?;

        self.log_and_trigger_event(event_sender, &updated_promotion)
            .await?;

        Ok(updated_promotion)
    }
}

impl DeactivatePromotionCommand {
    async fn deactivate_promotion(
        &self,
        db: &DatabaseConnection,
    ) -> Result<promotion_entity::Model, ServiceError> {
        let promotion = promotion_entity::Entity::find_by_id(self.promotion_id)
            .one(db)
            .await
            .map_err(|e| {
                error!("Failed to find promotion ID {}: {:?}", self.promotion_id, e);
                ServiceError::db_error(e)
            })?
            .ok_or_else(|| {
                error!("Promotion ID {} not found", self.promotion_id);
                ServiceError::NotFound(format!("Promotion ID {} not found", self.promotion_id))
            })?;

        let mut promotion_active_model: promotion_entity::ActiveModel = promotion.into();

        promotion_active_model.status = Set(PromotionStatus::Inactive);

        promotion_active_model.update(db).await.map_err(|e| {
            error!(
                "Failed to deactivate promotion ID {}: {:?}",
                self.promotion_id, e
            );
            ServiceError::db_error(e)
        })
    }

    async fn log_and_trigger_event(
        &self,
        event_sender: Arc<EventSender>,
        promotion: &promotion_entity::Model,
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
