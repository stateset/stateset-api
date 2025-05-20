use std::sync::Arc;
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
pub struct RefundPaymentCommand {
    pub payment_id: Uuid,
    #[validate(range(min = 0.01))]
    pub amount: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RefundPaymentResult {
    pub payment_id: Uuid,
}

#[async_trait]
impl Command for RefundPaymentCommand {
    type Result = RefundPaymentResult;

    #[instrument(skip(self, _db_pool, event_sender))]
    async fn execute(
        &self,
        _db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        self.validate()
            .map_err(|e| ServiceError::ValidationError(e.to_string()))?;

        info!(payment_id = %self.payment_id, amount = self.amount, "Payment refunded");

        event_sender
            .send(Event::PaymentRefunded(self.payment_id))
            .await
            .map_err(ServiceError::EventError)?;

        Ok(RefundPaymentResult { payment_id: self.payment_id })
    }
}
