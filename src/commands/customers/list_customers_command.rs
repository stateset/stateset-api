use uuid::Uuid;
use async_trait::async_trait;
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
    models::customer::{self, Entity as Customer},
};

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct ListCustomersCommand {
    #[validate(range(min = 1, max = 1000))]
    pub limit: Option<u64>,
    pub offset: Option<u64>,
}

#[async_trait]
impl Command for ListCustomersCommand {
    type Result = Vec<customer::Model>;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(
        &self,
        db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        self.validate()
            .map_err(|e| ServiceError::ValidationError(e.to_string()))?;

        let db = db_pool.as_ref();
        let mut query = Customer::find();

        if let Some(limit) = self.limit {
            query = query.limit(limit);
        }
        if let Some(offset) = self.offset {
            query = query.offset(offset);
        }

        let customers = query.all(db).await.map_err(|e| {
            let msg = format!("Failed to list customers: {}", e);
            error!("{}", msg);
            ServiceError::DatabaseError(msg)
        })?;

        info!(count = customers.len(), "Listed customers");
        event_sender
            .send(Event::with_data("customers_listed".to_string()))
            .await
            .map_err(ServiceError::EventError)?;

        Ok(customers)
    }
}
