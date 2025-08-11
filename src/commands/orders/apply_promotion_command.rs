use crate::{
    commands::Command,
    db::DbPool,
    errors::ServiceError,
    events::{Event, EventSender},
    models::{order_entity, promotion_entity, promotion_entity::PromotionStatus},
};
use async_trait::async_trait;
use lazy_static::lazy_static;
use prometheus::IntCounter;
use sea_orm::{*, Set, IntoActiveModel};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info, instrument, warn};
use uuid::Uuid;
use validator::Validate;

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct ApplyPromotionToOrderCommand {
    pub order_id: Uuid,
    pub promotion_id: Uuid,
}

#[async_trait::async_trait]
impl Command for ApplyPromotionToOrderCommand {
    type Result = order_entity::Model;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(
        &self,
        db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        let db = db_pool.clone();

        let promotion = self.fetch_promotion(&db).await?;

        self.validate_promotion(&promotion)?;

        let updated_order = self.apply_promotion_to_order(&db, &promotion).await?;

        self.log_and_trigger_event(event_sender, updated_order.clone())
            .await?;

        Ok(updated_order)
    }
}

impl ApplyPromotionToOrderCommand {
    async fn fetch_promotion(
        &self,
        db: &DatabaseConnection,
    ) -> Result<promotion_entity::Model, ServiceError> {
        promotion_entity::Entity::find_by_id(self.promotion_id)
            .one(db)
            .await
            .map_err(|e| {
                error!(
                    "Failed to fetch promotion ID {}: {:?}",
                    self.promotion_id, e
                );
                ServiceError::DatabaseError(e)
            })?
            .ok_or_else(|| {
                warn!("Promotion ID {} not found", self.promotion_id);
                ServiceError::NotFound(format!("Promotion ID {} not found", self.promotion_id))
            })
    }

    fn validate_promotion(&self, promotion: &promotion_entity::Model) -> Result<(), ServiceError> {
        if promotion.status != PromotionStatus::Active {
            error!("Promotion ID {} is not active", self.promotion_id);
            return Err(ServiceError::ValidationError(
                "Promotion is not active".to_string(),
            ));
        }
        Ok(())
    }

    async fn apply_promotion_to_order(
        &self,
        db: &DatabaseConnection,
        promotion: &promotion_entity::Model,
    ) -> Result<order_entity::Model, ServiceError> {
        let order = order_entity::Entity::find_by_id(self.order_id)
            .one(db)
            .await
            .map_err(|e| {
                error!("Failed to find Order ID {}: {:?}", self.order_id, e);
                ServiceError::DatabaseError(e)
            })?
            .ok_or_else(|| {
                error!("Order ID {} not found", self.order_id);
                ServiceError::NotFound(format!("Order ID {} not found", self.order_id))
            })?;

        let mut active_model: order_entity::ActiveModel = order.into();
        // TODO: Apply promotion logic
        active_model.discount_amount = Set(Some(promotion.max_discount_amount));

        // Update the order
        let updated_order = active_model.update(db).await.map_err(|e| {
            error!("Failed to update order: {}", e);
            ServiceError::DatabaseError(e)
        })?;

        Ok(updated_order)
    }

    async fn log_and_trigger_event(
        &self,
        event_sender: Arc<EventSender>,
        _updated_order: order_entity::Model,
    ) -> Result<(), ServiceError> {
        info!(
            "Promotion ID {} applied to order ID {}",
            self.promotion_id, self.order_id
        );

        event_sender
            .send(Event::OrderUpdated(self.order_id))
            .await
            .map_err(|e| {
                error!(
                    "Failed to send OrderUpdated event for order ID {}: {:?}",
                    self.order_id, e
                );
                ServiceError::EventError(e.to_string())
            })
    }
}
