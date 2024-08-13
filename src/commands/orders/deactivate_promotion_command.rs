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
pub struct DeactivatePromotionCommand {
    pub promotion_id: i32,
}

#[async_trait]
impl Command for DeactivatePromotionCommand {
    type Result = Promotion;

    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;

        // Deactivate the promotion
        let updated_promotion = diesel::update(promotions::table.find(self.promotion_id))
            .set(promotions::status.eq(PromotionStatus::Inactive))
            .get_result::<Promotion>(&conn)
            .map_err(|e| ServiceError::DatabaseError)?;

        // Log and trigger events
        info!("Promotion deactivated: {}", self.promotion_id);
        event_sender.send(Event::PromotionDeactivated(self.promotion_id)).await.map_err(|e| ServiceError::EventError(e.to_string()))?;

        Ok(updated_promotion)
    }
}