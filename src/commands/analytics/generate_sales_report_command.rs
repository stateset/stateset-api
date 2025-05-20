use std::sync::Arc;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use validator::Validate;
use async_trait::async_trait;
use tracing::{info, instrument};

use crate::{
    db::DbPool,
    errors::ServiceError,
    events::{Event, EventSender},
    commands::Command,
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

    #[instrument(skip(self, _db_pool, event_sender))]
    async fn execute(
        &self,
        _db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        self.validate()
            .map_err(|e| ServiceError::ValidationError(e.to_string()))?;

        // Placeholder total sales value
        let total = 0.0;
        info!("Generating sales report");

        event_sender
            .send(Event::with_data("sales_report_generated".to_string()))
            .await
            .map_err(ServiceError::EventError)?;

        Ok(GenerateSalesReportResult { total_sales: total })
    }
}
