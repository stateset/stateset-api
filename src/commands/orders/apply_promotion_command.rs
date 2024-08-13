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
pub struct ApplyPromotionToOrderCommand {
    pub order_id: i32,
    pub promotion_id: i32,
}

#[async_trait]
impl Command for ApplyPromotionToOrderCommand {
    type Result = Order;

    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;

        // Fetch the promotion and validate its applicability
        let promotion = promotions::table
            .find(self.promotion_id)
            .first::<Promotion>(&conn)
            .map_err(|_| ServiceError::NotFound)?;

        if promotion.status != PromotionStatus::Active {
            return Err(ServiceError::ValidationError("Promotion is not active".to_string()));
        }

        // Apply the promotion to the order
        let updated_order = apply_promotion_to_order(self.order_id, &promotion, &conn)?;

        // Log and trigger events
        info!("Promotion applied to order ID: {}", self.order_id);
        event_sender.send(Event::OrderUpdated(self.order_id)).await.map_err(|e| ServiceError::EventError(e.to_string()))?;

        Ok(updated_order)
    }
}