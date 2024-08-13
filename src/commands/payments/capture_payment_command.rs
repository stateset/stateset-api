
#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct CapturePaymentCommand {
    pub order_id: i32,
    #[validate(range(min = 0.01))]
    pub amount: f64,
}

#[async_trait]
impl Command for CapturePaymentCommand {
    type Result = Payment;

    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;

        // Capture payment (this could involve an external payment gateway)
        let payment = capture_payment(self.order_id, self.amount)
            .await
            .map_err(|e| ServiceError::PaymentProcessingError(e.to_string()))?;

        // Log and trigger events
        info!("Payment captured for order ID: {}. Amount: {}", self.order_id, self.amount);
        event_sender.send(Event::PaymentCaptured(self.order_id, self.amount)).await.map_err(|e| ServiceError::EventError(e.to_string()))?;

        Ok(payment)
    }
}
