use crate::{errors::AppError, models::payment};
use std::sync::Arc;
use sea_orm::{DatabaseConnection, ActiveModelTrait, Set};
use chrono::Utc;
use uuid::Uuid;

pub struct PaymentService {
    db: Arc<DatabaseConnection>,
}

impl PaymentService {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }

    /// Process a payment for an order and persist the payment record.
    pub async fn process_payment(
        &self,
        order_id: uuid::Uuid,
        amount: rust_decimal::Decimal,
    ) -> Result<Uuid, AppError> {
        tracing::info!(%order_id, %amount, "processing payment");

        let model = payment::ActiveModel {
            id: Set(Uuid::new_v4()),
            order_id: Set(order_id),
            amount: Set(amount),
            status: Set(Some("Processed".to_string())),
            created_at: Set(Utc::now()),
        };

        let result = model.insert(&*self.db).await?;
        Ok(result.id)
    }
}