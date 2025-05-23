use std::sync::Arc;
use sea_orm::{DatabaseConnection, ActiveModelTrait, Set};
use rust_decimal::Decimal;
use uuid::Uuid;
use chrono::Utc;

use crate::{
    errors::AppError,
    models::cash_sale,
};

pub struct CashSaleService {
    db: Arc<DatabaseConnection>,
}

impl CashSaleService {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }

    pub async fn create_cash_sale(
        &self,
        order_id: Uuid,
        amount: Decimal,
        payment_method: String,
    ) -> Result<Uuid, AppError> {
        let model = cash_sale::ActiveModel {
            id: Set(Uuid::new_v4()),
            order_id: Set(order_id),
            amount: Set(amount),
            payment_method: Set(payment_method),
            created_at: Set(Utc::now()),
        };
        let result = model.insert(&*self.db).await?;
        Ok(result.id)
    }
}
