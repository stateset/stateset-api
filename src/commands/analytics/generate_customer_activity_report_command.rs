use std::sync::Arc;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
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

    #[instrument(skip(self, _db_pool, event_sender))]
    async fn execute(
        &self,
        _db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        self.validate()
            .map_err(|e| ServiceError::ValidationError(e.to_string()))?;

        // Placeholder orders count
        let orders = 0usize;
        info!(customer_id = %self.customer_id, "Generating customer activity report");

        event_sender
            .send(Event::with_data("customer_activity_report_generated".to_string()))
            .await
            .map_err(ServiceError::EventError)?;

        Ok(GenerateCustomerActivityReportResult { customer_id: self.customer_id, orders })
    }
}

