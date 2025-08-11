use crate::{errors::AppError, models::invoices};
use chrono::Utc;
use sea_orm::{ActiveModelTrait, DatabaseConnection, Set};
use std::sync::Arc;

pub struct InvoicingService {
    db: Arc<DatabaseConnection>,
}

impl InvoicingService {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }

    /// Generate an invoice for the given order and persist it to the database.
    pub async fn generate_invoice(&self, order_id: uuid::Uuid) -> Result<uuid::Uuid, AppError> {
        let invoice_id = uuid::Uuid::new_v4();
        tracing::info!(%order_id, %invoice_id, "generate invoice");

        let model = invoices::ActiveModel {
            id: Set(invoice_id.to_string()),
            order_id: Set(Some(order_id.to_string())),
            created: Set(Some(Utc::now())),
            status: Set(Some("Draft".to_string())),
            ..Default::default()
        };

        model.insert(&*self.db).await?;

        Ok(invoice_id)
    }
}
