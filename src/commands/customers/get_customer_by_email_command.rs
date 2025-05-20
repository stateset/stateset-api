use std::sync::Arc;
use sea_orm::*;
use serde::{Deserialize, Serialize};
use async_trait::async_trait;
use tracing::{error, info, instrument};
use validator::Validate;

use crate::{
    db::DbPool,
    errors::ServiceError,
    events::{Event, EventSender},
    models::customer::{self, Entity as Customer},
    commands::Command,
};

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct GetCustomerByEmailCommand {
    #[validate(email)]
    pub email: String,
}

#[async_trait]
impl Command for GetCustomerByEmailCommand {
    type Result = Option<customer::Model>;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(
        &self,
        db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        self.validate()
            .map_err(|e| ServiceError::ValidationError(e.to_string()))?;

        let db = db_pool.as_ref();
        let customer = Customer::find()
            .filter(customer::Column::Email.eq(self.email.clone()))
            .one(db)
            .await
            .map_err(|e| {
                let msg = format!("Failed to get customer by email: {}", e);
                error!("{}", msg);
                ServiceError::DatabaseError(msg)
            })?;

        info!("Fetched customer by email: {}", self.email);
        event_sender
            .send(Event::with_data(format!("customer_fetched_by_email:{}", self.email)))
            .await
            .map_err(ServiceError::EventError)?;

        Ok(customer)
    }
}
