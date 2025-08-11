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
pub struct SearchCustomersCommand {
    #[validate(length(min = 1))]
    pub term: String,
}

#[async_trait]
impl Command for SearchCustomersCommand {
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
        let pattern = format!("%{}%", self.term);

        let customers = Customer::find()
            .filter(
                Condition::any()
                    .add(customer::Column::Name.like(&pattern))
                    .add(customer::Column::Email.like(&pattern)),
            )
            .all(db)
            .await
            .map_err(|e| {
                let msg = format!("Failed to search customers: {}", e);
                error!("{}", msg);
                ServiceError::DatabaseError(msg)
            })?;

        info!(
            count = customers.len(),
            term = self.term,
            "Searched customers"
        );
        event_sender
            .send(Event::with_data("customers_searched".to_string()))
            .await
            .map_err(ServiceError::EventError)?;

        Ok(customers)
    }
}
