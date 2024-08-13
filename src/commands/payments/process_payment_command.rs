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
pub struct ProcessPaymentCommand {
    pub order_id: i32,
    pub payment_method: PaymentMethod, // Enum representing the payment method
    pub amount: f64,
}

#[async_trait]
impl Command for ProcessPaymentCommand {
    type Result = Payment;

    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        // Validate payment method, ensure it's supported
        self.payment_method.validate().map_err(|e| ServiceError::ValidationError(e.to_string()))?;

        let conn = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;

        // Assume there's a service or API to process the payment
        let payment = PaymentService::process(self.order_id, self.payment_method.clone(), self.amount)
            .await
            .map_err(|e| ServiceError::PaymentProcessingError(e.to_string()))?;

        // Save the payment record to the database
        let saved_payment = diesel::insert_into(payments::table)
            .values(&payment)
            .get_result::<Payment>(&conn)
            .map_err(|e| ServiceError::DatabaseError)?;

        // Trigger an event
        event_sender.send(Event::PaymentProcessed(payment.id)).await.map_err(|e| ServiceError::EventError(e.to_string()))?;

        // Log the payment processing
        info!("Payment processed: {:?}", saved_payment);

        Ok(saved_payment)
    }
}