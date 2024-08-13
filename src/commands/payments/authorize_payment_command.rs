use async_trait::async_trait;
use serde::{Serialize, Deserialize};
use std::sync::Arc;
use crate::{errors::ServiceError, db::DbPool, models::{OrderItem}};
use crate::events::{Event, EventSender};
use validator::Validate;
use tracing::{info, error, instrument};
use diesel::prelude::*;
use prometheus::IntCounter;


#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct AuthorizePaymentCommand {
    pub order_id: i32,
    #[validate(range(min = 0.01))]
    pub amount: f64,
    #[validate(length(min = 1))]
    pub payment_method: String, // Payment method details (could be a token or an ID)
}

#[async_trait]
impl Command for AuthorizePaymentCommand {
    type Result = PaymentAuthorization;

    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;

        // Authorize payment (this could involve an external payment gateway)
        let payment_authorization = authorize_payment(self.order_id, self.amount, &self.payment_method)
            .await
            .map_err(|e| ServiceError::PaymentProcessingError(e.to_string()))?;

        // Log and trigger events
        info!("Payment authorized for order ID: {}. Amount: {}", self.order_id, self.amount);
        event_sender.send(Event::PaymentAuthorized(self.order_id, self.amount)).await.map_err(|e| ServiceError::EventError(e.to_string()))?;

        Ok(payment_authorization)
    }
}
