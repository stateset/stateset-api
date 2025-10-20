use uuid::Uuid;
use crate::models::order_entity::{self, Entity as Order};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sea_orm::*;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info, instrument};
use validator::Validate;

use crate::{
    commands::Command,
    db::DbPool,
    errors::ServiceError,
    events::{Event, EventSender},
};

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct GenerateSalesReportCommand {
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GenerateSalesReportResult {
    pub total_sales: f64,
}

#[async_trait]
impl Command for GenerateSalesReportCommand {
    type Result = GenerateSalesReportResult;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(
        &self,
        db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        self.validate()
            .map_err(|e| ServiceError::ValidationError(e.to_string()))?;

        let db = db_pool.as_ref();

        let total = Order::find()
            .filter(
                Condition::all()
                    .add(order_entity::Column::CreatedAt.gte(self.start.naive_utc()))
                    .add(order_entity::Column::CreatedAt.lte(self.end.naive_utc())),
            )
            .sum::<f64>(order_entity::Column::TotalAmount)
            .await
            .map_err(|e| {
                error!("Failed to calculate total sales: {}", e);
                ServiceError::db_error(e)
            })?
            .unwrap_or(0.0);

        info!(start = %self.start, end = %self.end, total_sales = total, "Generating sales report");

        event_sender
            .send(Event::with_data("sales_report_generated".to_string()))
            .await
            .map_err(ServiceError::EventError)?;

        Ok(GenerateSalesReportResult { total_sales: total })
    }
}
