use async_trait::async_trait;
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
pub struct CreatePaymentCommand {
    pub order_id: Uuid,
    #[validate(range(min = 0.01))]
    pub amount: f64,
    #[validate(length(min = 3, max = 3))]
    pub currency: String,
    pub payment_method_id: Uuid,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreatePaymentResult {
    pub payment_id: Uuid,
}

#[async_trait]
impl Command for CreatePaymentCommand {
    type Result = CreatePaymentResult;

    #[instrument(skip(self, _db_pool, event_sender))]
    async fn execute(
        &self,
        _db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        self.validate()
            .map_err(|e| ServiceError::ValidationError(e.to_string()))?;

        let payment_id = Uuid::new_v4();
        info!(payment_id = %payment_id, order_id = %self.order_id, "Payment authorized");

        event_sender
            .send(Event::PaymentAuthorized(payment_id))
            .await
            .map_err(ServiceError::EventError)?;

        Ok(CreatePaymentResult { payment_id })
    }
}
