use async_trait::async_trait;
use serde::{Serialize, Deserialize};
use std::sync::Arc;
use crate::{errors::ServiceError, db::DbPool, models::{Order, OrderItem, OrderStatus}};
use crate::events::{Event, EventSender};
use validator::Validate;
use tracing::{info, error, instrument};
use chrono::{DateTime, Utc};
use diesel::prelude::*;
use prometheus::IntCounter;

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct CreatePromotionCommand {
    #[validate(length(min = 1))]
    pub name: String,

    #[validate(range(min = 0.0))]
    pub discount_percentage: f64,

    #[validate]
    pub start_date: chrono::NaiveDateTime,

    #[validate]
    pub end_date: chrono::NaiveDateTime,

    pub applicable_products: Option<Vec<i32>>, // Product IDs this promotion applies to
}

#[async_trait]
impl Command for CreatePromotionCommand {
    type Result = Promotion;

    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;

        // Create a new promotion
        let promotion = Promotion {
            name: self.name.clone(),
            discount_percentage: self.discount_percentage,
            start_date: self.start_date,
            end_date: self.end_date,
            status: PromotionStatus::Active,
            // Other fields like applicable_products, creation timestamps, etc.
        };

        let saved_promotion = diesel::insert_into(promotions::table)
            .values(&promotion)
            .get_result::<Promotion>(&conn)
            .map_err(|e| ServiceError::DatabaseError)?;

        // Log and trigger events
        info!("Promotion created: {:?}", saved_promotion);
        event_sender.send(Event::PromotionCreated(saved_promotion.id)).await.map_err(|e| ServiceError::EventError(e.to_string()))?;

        Ok(saved_promotion)
    }
}
