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
pub struct VoidPaymentCommand {
    pub payment_id: Uuid,
    /// Optional reason for voiding the payment
    #[validate(length(min = 1))]
    pub reason: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct VoidPaymentResult {
    pub payment_id: Uuid,
}

#[async_trait]
impl Command for VoidPaymentCommand {
    type Result = VoidPaymentResult;

    #[instrument(skip(self, _db_pool, event_sender))]
    async fn execute(
        &self,
        _db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        self.validate()
            .map_err(|e| ServiceError::ValidationError(e.to_string()))?;

        info!(payment_id = %self.payment_id, "Payment voided");

        event_sender
            .send(Event::PaymentVoided(self.payment_id))
            .await
            .map_err(ServiceError::EventError)?;

        Ok(VoidPaymentResult {
            payment_id: self.payment_id,
        })
    }
}
