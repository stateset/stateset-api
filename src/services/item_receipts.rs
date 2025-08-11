use chrono::Utc;
use sea_orm::{ActiveModelTrait, DatabaseConnection, Set};
use std::sync::Arc;
use uuid::Uuid;

use crate::{errors::AppError, models::item_receipt};

pub struct ItemReceiptService {
    db: Arc<DatabaseConnection>,
}

impl ItemReceiptService {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }

    pub async fn record_receipt(
        &self,
        purchase_order_id: Option<Uuid>,
        product_id: Uuid,
        warehouse_id: Uuid,
        quantity: i32,
        notes: Option<String>,
    ) -> Result<Uuid, AppError> {
        let model = item_receipt::ActiveModel {
            id: Set(Uuid::new_v4()),
            purchase_order_id: Set(purchase_order_id),
            product_id: Set(product_id),
            warehouse_id: Set(warehouse_id),
            quantity: Set(quantity),
            received_at: Set(Utc::now()),
            notes: Set(notes),
        };
        let result = model.insert(&*self.db).await?;
        Ok(result.id)
    }
}
