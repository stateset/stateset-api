use crate::{
    commands::Command,
    db::DbPool,
    errors::ServiceError,
    events::{Event, EventSender},
    models::{order_entity, promotion_entity, promotion_entity::PromotionStatus},
    services::promotions::PromotionService,
};
use async_trait::async_trait;
use chrono::Utc;
use rust_decimal::prelude::ToPrimitive;
use sea_orm::{Set, DatabaseConnection, EntityTrait};
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
                ServiceError::db_error(e)
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
                ServiceError::db_error(e)
            })?
            .ok_or_else(|| {
                error!("Order ID {} not found", self.order_id);
                ServiceError::NotFound(format!("Order ID {} not found", self.order_id))
            })?;

        // Check if order already has a promotion applied
        if order.promotion_id.is_some() {
            return Err(ServiceError::ValidationError(
                "Order already has a promotion applied. Remove it first before applying a new one.".to_string(),
            ));
        }

        // Check minimum order amount requirement
        if let Some(min_amount) = promotion.min_order_amount {
            let min_amount_f64 = min_amount.to_f64().unwrap_or(0.0);
            if order.total_amount < min_amount_f64 {
                return Err(ServiceError::ValidationError(format!(
                    "Order total ${:.2} is below minimum required amount ${:.2} for this promotion",
                    order.total_amount, min_amount_f64
                )));
            }
        }

        // Check promotion usage limit
        if let Some(limit) = promotion.usage_limit {
            if promotion.usage_count >= limit {
                return Err(ServiceError::ValidationError(
                    "Promotion has reached its usage limit".to_string(),
                ));
            }
        }

        // Check promotion validity period
        let now = Utc::now();
        if now < promotion.start_date {
            return Err(ServiceError::ValidationError(
                "Promotion has not started yet".to_string(),
            ));
        }
        if now > promotion.end_date {
            return Err(ServiceError::ValidationError(
                "Promotion has expired".to_string(),
            ));
        }

        // Calculate discount using PromotionService logic
        let promotion_service = PromotionService::new(db.clone());
        let subtotal_cents = (order.total_amount * 100.0) as i64;
        let discount_cents = promotion_service.calculate_discount(promotion, subtotal_cents)?;
        let discount_amount = (discount_cents as f64) / 100.0;

        info!(
            order_id = %self.order_id,
            promotion_id = %self.promotion_id,
            promotion_type = ?promotion.promotion_type,
            original_total = order.total_amount,
            discount_amount = discount_amount,
            "Calculated promotion discount"
        );

        let mut active_model: order_entity::ActiveModel = order.into();
        active_model.discount_amount = Set(Some(discount_amount));
        active_model.promotion_id = Set(Some(self.promotion_id));
        active_model.updated_at = Set(Utc::now());

        // Update the order
        let updated_order = active_model.update(db).await.map_err(|e| {
            error!("Failed to update order: {}", e);
            ServiceError::db_error(e)
        })?;

        // Increment promotion usage count
        if let Err(e) = promotion_service.increment_usage_count(self.promotion_id).await {
            warn!(
                "Failed to increment promotion usage count for {}: {}",
                self.promotion_id, e
            );
            // Don't fail the whole operation for this
        }

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
            .send(Event::OrderUpdated {
                order_id: self.order_id,
                checkout_session_id: None,
                status: None,
                refunds: vec![],
            })
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
