use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{info, instrument};
use uuid::Uuid;
use validator::Validate;

use crate::{
    commands::Command,
    db::DbPool,
    errors::ServiceError,
    events::{Event, EventSender},
};

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct DemandForecastCommand {
    pub product_id: Uuid,
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DemandForecastResult {
    pub product_id: Uuid,
    pub predicted_demand: i32,
}

#[async_trait]
impl Command for DemandForecastCommand {
    type Result = DemandForecastResult;

    #[instrument(skip(self, _db_pool, event_sender))]
    async fn execute(
        &self,
        _db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        self.validate()
            .map_err(|e| ServiceError::ValidationError(e.to_string()))?;

        info!(product_id = %self.product_id, "Running demand forecast");

        // Placeholder predicted demand value
        let predicted = 100;
        event_sender
            .send(Event::with_data(format!(
                "demand_forecast:{}:{}",
                self.product_id, predicted
            )))
            .await
            .map_err(ServiceError::EventError)?;

        Ok(DemandForecastResult {
            product_id: self.product_id,
            predicted_demand: predicted,
        })
    }
}
