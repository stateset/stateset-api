use uuid::Uuid;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info, instrument};
use validator::Validate;
use rust_decimal::Decimal;

use crate::{
    commands::Command,
    db::DbPool,
    errors::ServiceError,
    events::{Event, EventSender},
    models::{promotion_entity::{self, Entity as Promotion, PromotionType}, PromotionStatus},
};
use sea_orm::{ActiveModelTrait, ActiveValue::Set, DatabaseConnection, EntityTrait, TransactionError, TransactionTrait};
use chrono::{DateTime, Utc};

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct CreatePromotionCommand {
    #[validate(length(min = 1))]
    pub name: String,

    pub description: Option<String>,

    #[validate(length(min = 1))]
    pub promotion_code: String,

    pub promotion_type: String, // Will be converted to enum

    #[validate(range(min = 0.0))]
    pub discount_value: f64,

    pub min_order_amount: Option<f64>,

    pub max_discount_amount: Option<f64>,

    pub usage_limit: Option<i32>,

    pub start_date: DateTime<Utc>,

    pub end_date: DateTime<Utc>,
}

#[async_trait::async_trait]
impl Command for CreatePromotionCommand {
    type Result = promotion_entity::Model;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(
        &self,
        db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        self.validate_promotion_dates()?;

        let db = db_pool.as_ref();

        let promotion_id = Uuid::new_v4();

        // Convert promotion type string to enum
        let promotion_type = match self.promotion_type.as_str() {
            "Percentage" => PromotionType::Percentage,
            "FixedAmount" => PromotionType::FixedAmount,
            "BuyOneGetOne" => PromotionType::BuyOneGetOne,
            "FreeShipping" => PromotionType::FreeShipping,
            _ => PromotionType::Percentage, // Default
        };

        let promotion = promotion_entity::ActiveModel {
            id: Set(promotion_id),
            name: Set(self.name.clone()),
            description: Set(self.description.clone()),
            promotion_code: Set(self.promotion_code.clone()),
            promotion_type: Set(promotion_type),
            discount_value: Set(Decimal::from_f64_retain(self.discount_value).unwrap_or_default()),
            min_order_amount: Set(self.min_order_amount.map(|v| Decimal::from_f64_retain(v).unwrap_or_default())),
            max_discount_amount: Set(self.max_discount_amount.map(|v| Decimal::from_f64_retain(v).unwrap_or_default())),
            usage_limit: Set(self.usage_limit),
            usage_count: Set(0),
            start_date: Set(self.start_date),
            end_date: Set(self.end_date),
            status: Set(PromotionStatus::Active),
            created_at: Set(chrono::Utc::now()),
            updated_at: Set(chrono::Utc::now()),
        };

        let saved_promotion = self.save_promotion(db, promotion).await?;

        self.log_and_trigger_event(event_sender, &saved_promotion)
            .await?;

        Ok(saved_promotion)
    }
}

impl CreatePromotionCommand {
    fn validate_promotion_dates(&self) -> Result<(), ServiceError> {
        if self.start_date >= self.end_date {
            error!("Promotion start date must be before end date");
            return Err(ServiceError::ValidationError(
                "Promotion start date must be before end date".to_string(),
            ));
        }
        Ok(())
    }

    async fn save_promotion(
        &self,
        db: &DatabaseConnection,
        promotion: promotion_entity::ActiveModel,
    ) -> Result<promotion_entity::Model, ServiceError> {
        promotion.insert(db).await.map_err(|e| {
            error!("Failed to save promotion: {:?}", e);
            ServiceError::db_error(e)
        })
    }

    async fn log_and_trigger_event(
        &self,
        event_sender: Arc<EventSender>,
        promotion: &promotion_entity::Model,
    ) -> Result<(), ServiceError> {
        info!("Promotion created: {:?}", promotion);

        event_sender
            .send(Event::PromotionCreated(promotion.id))
            .await
            .map_err(|e| {
                error!("Failed to send PromotionCreated event: {:?}", e);
                ServiceError::EventError(e.to_string())
            })
    }
}
