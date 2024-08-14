use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info, instrument};
use validator::Validate;

use crate::{
    db::DbPool,
    errors::ServiceError,
    events::{Event, EventSender},
    models::{Order, OrderItem, OrderStatus, Promotion, PromotionStatus},
};
use diesel::prelude::*;
use chrono::{DateTime, Utc};
use prometheus::IntCounter;

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct ApplyPromotionToOrderCommand {
    pub order_id: i32,
    pub promotion_id: i32,
}

#[async_trait]
impl Command for ApplyPromotionToOrderCommand {
    type Result = Order;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|e| {
            error!("Failed to get database connection: {:?}", e);
            ServiceError::DatabaseError
        })?;

        let promotion = self.fetch_promotion(&conn)?;

        self.validate_promotion(&promotion)?;

        let updated_order = self.apply_promotion_to_order(&conn, &promotion)?;

        self.log_and_trigger_event(event_sender, updated_order.clone()).await?;

        Ok(updated_order)
    }
}

impl ApplyPromotionToOrderCommand {
    fn fetch_promotion(&self, conn: &PgConnection) -> Result<Promotion, ServiceError> {
        promotions::table
            .find(self.promotion_id)
            .first::<Promotion>(conn)
            .map_err(|e| {
                error!("Failed to fetch promotion ID {}: {:?}", self.promotion_id, e);
                ServiceError::NotFound
            })
    }

    fn validate_promotion(&self, promotion: &Promotion) -> Result<(), ServiceError> {
        if promotion.status != PromotionStatus::Active {
            error!("Promotion ID {} is not active", self.promotion_id);
            return Err(ServiceError::ValidationError("Promotion is not active".to_string()));
        }
        Ok(())
    }

    fn apply_promotion_to_order(
        &self,
        conn: &PgConnection,
        promotion: &Promotion,
    ) -> Result<Order, ServiceError> {
        diesel::update(orders::table.find(self.order_id))
            .set((
                orders::promotion_id.eq(Some(promotion.id)),
                orders::discount_amount.eq(Some(promotion.discount_amount)),
            ))
            .get_result::<Order>(conn)
            .map_err(|e| {
                error!(
                    "Failed to apply promotion ID {} to order ID {}: {:?}",
                    self.promotion_id, self.order_id, e
                );
                ServiceError::DatabaseError
            })
    }

    async fn log_and_trigger_event(
        &self,
        event_sender: Arc<EventSender>,
        updated_order: Order,
    ) -> Result<(), ServiceError> {
        info!("Promotion ID {} applied to order ID {}", self.promotion_id, self.order_id);

        event_sender
            .send(Event::OrderUpdated(self.order_id))
            .await
            .map_err(|e| {
                error!("Failed to send OrderUpdated event for order ID {}: {:?}", self.order_id, e);
                ServiceError::EventError(e.to_string())
            })
    }
}
