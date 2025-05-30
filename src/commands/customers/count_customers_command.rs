use std::sync::Arc;
use sea_orm::*;
use serde::{Deserialize, Serialize};
use async_trait::async_trait;
use tracing::{error, info, instrument};

use crate::{
    db::DbPool,
    errors::ServiceError,
    events::{Event, EventSender},
    models::customer::{self, Entity as Customer},
    commands::Command,
};

#[derive(Debug, Serialize, Deserialize)]
pub struct CountCustomersCommand;

#[async_trait]
impl Command for CountCustomersCommand {
    type Result = u64;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(
        &self,
        db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        let db = db_pool.as_ref();
        let count = Customer::find()
            .count(db)
            .await
            .map_err(|e| {
                let msg = format!("Failed to count customers: {}", e);
                error!("{}", msg);
                ServiceError::DatabaseError(msg)
            })?;

        info!(count, "Counted customers");
        event_sender
            .send(Event::with_data("customers_counted".to_string()))
            .await
            .map_err(ServiceError::EventError)?;

        Ok(count)
    }
}
