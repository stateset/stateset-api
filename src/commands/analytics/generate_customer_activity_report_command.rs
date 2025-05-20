use std::sync::Arc;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;
use async_trait::async_trait;
use tracing::{error, info, instrument};
use sea_orm::*;
use crate::models::order_entity::{self, Entity as Order};

use crate::{
    db::DbPool,
    errors::ServiceError,
    events::{Event, EventSender},
    commands::Command,
};

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct GenerateCustomerActivityReportCommand {
    pub customer_id: Uuid,
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GenerateCustomerActivityReportResult {
    pub customer_id: Uuid,
    pub orders: usize,
}

#[async_trait]
impl Command for GenerateCustomerActivityReportCommand {
    type Result = GenerateCustomerActivityReportResult;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(
        &self,
        db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        self.validate()
            .map_err(|e| ServiceError::ValidationError(e.to_string()))?;

        let db = db_pool.as_ref();

        let orders = Order::find()
            .filter(order_entity::Column::CustomerId.eq(self.customer_id))
            .filter(order_entity::Column::CreatedAt.gte(self.start.naive_utc()))
            .filter(order_entity::Column::CreatedAt.lte(self.end.naive_utc()))
            .count(db)
            .await
            .map_err(|e| {
                error!("Failed to count orders for customer {}: {}", self.customer_id, e);
                ServiceError::DatabaseError(e.to_string())
            })?;

        info!(customer_id = %self.customer_id, orders, "Generating customer activity report");

        event_sender
            .send(Event::with_data("customer_activity_report_generated".to_string()))
            .await
            .map_err(ServiceError::EventError)?;

        Ok(GenerateCustomerActivityReportResult { customer_id: self.customer_id, orders: orders as usize })
    }
}

