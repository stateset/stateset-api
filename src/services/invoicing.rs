use crate::errors::AppError;
use std::sync::Arc;
use sea_orm::DatabaseConnection;

pub struct InvoicingService {
    db: Arc<DatabaseConnection>,
}

impl InvoicingService {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }

    /// Generate an invoice for the given order. The invoice is not
    /// persisted yet and a UUID is returned for reference.
    pub async fn generate_invoice(
        &self,
        order_id: uuid::Uuid,
    ) -> Result<uuid::Uuid, AppError> {
        let invoice_id = uuid::Uuid::new_v4();
        tracing::info!(%order_id, %invoice_id, "generate invoice");
        // TODO: persist invoice to database
        Ok(invoice_id)
    }
}