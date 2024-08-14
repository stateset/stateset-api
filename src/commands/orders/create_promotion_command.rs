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
use chrono::{NaiveDateTime};

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct CreatePromotionCommand {
    #[validate(length(min = 1))]
    pub name: String,

    #[validate(range(min = 0.0))]
    pub discount_percentage: f64,

    #[validate]
    pub start_date: NaiveDateTime,

    #[validate]
    pub end_date: NaiveDateTime,

    pub applicable_products: Option<Vec<i32>>, // Product IDs this promotion applies to
}

#[async_trait]
impl Command for CreatePromotionCommand {
    type Result = Promotion;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        self.validate_promotion_dates()?;
        
        let conn = db_pool.get().map_err(|e| {
            error!("Failed to get database connection: {:?}", e);
            ServiceError::DatabaseError
        })?;

        let promotion = self.build_promotion();

        let saved_promotion = self.save_promotion(&conn, &promotion)?;

        self.log_and_trigger_event(event_sender, &saved_promotion).await?;

        Ok(saved_promotion)
    }
}

impl CreatePromotionCommand {
    fn validate_promotion_dates(&self) -> Result<(), ServiceError> {
        if self.start_date >= self.end_date {
            error!("Promotion start date must be before end date");
            return Err(ServiceError::ValidationError("Promotion start date must be before end date".to_string()));
        }
        Ok(())
    }

    fn build_promotion(&self) -> Promotion {
        Promotion {
            name: self.name.clone(),
            discount_percentage: self.discount_percentage,
            start_date: self.start_date,
            end_date: self.end_date,
            status: PromotionStatus::Active,
            // Include applicable_products and other fields as necessary
        }
    }

    fn save_promotion(&self, conn: &PgConnection, promotion: &Promotion) -> Result<Promotion, ServiceError> {
        diesel::insert_into(promotions::table)
            .values(promotion)
            .get_result::<Promotion>(conn)
            .map_err(|e| {
                error!("Failed to save promotion: {:?}", e);
                ServiceError::DatabaseError
            })
    }

    async fn log_and_trigger_event(&self, event_sender: Arc<EventSender>, promotion: &Promotion) -> Result<(), ServiceError> {
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
