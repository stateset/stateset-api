use crate::errors::AppError;
use std::sync::Arc;
use sea_orm::DatabaseConnection;

pub struct PaymentService {
    db: Arc<DatabaseConnection>,
}

impl PaymentService {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }

    /// Process a payment for an order. This is a stub implementation
    /// that simply logs the payment attempt.
    pub async fn process_payment(
        &self,
        order_id: uuid::Uuid,
        amount: rust_decimal::Decimal,
    ) -> Result<(), AppError> {
        tracing::info!(%order_id, %amount, "processing payment");
        // TODO: persist payment record and integrate with payment gateway
        Ok(())
    }
}