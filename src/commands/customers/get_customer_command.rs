use async_trait::async_trait;
use sea_orm::*;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info, instrument};
use uuid::Uuid;

use crate::{
    commands::Command,
    db::DbPool,
    errors::ServiceError,
    events::{Event, EventSender},
    models::customer::{self, Entity as Customer},
};

#[derive(Debug, Serialize, Deserialize)]
pub struct GetCustomerCommand {
    pub id: Uuid,
}

#[async_trait]
impl Command for GetCustomerCommand {
    type Result = Option<customer::Model>;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(
        &self,
        db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        let db = db_pool.as_ref();
        let customer = Customer::find_by_id(self.id).one(db).await.map_err(|e| {
            let msg = format!("Failed to get customer: {}", e);
            error!("{}", msg);
            ServiceError::DatabaseError(msg)
        })?;

        info!("Fetched customer: {}", self.id);
        event_sender
            .send(Event::with_data(format!("customer_fetched:{}", self.id)))
            .await
            .map_err(ServiceError::EventError)?;

        Ok(customer)
    }
}
