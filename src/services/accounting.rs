use crate::errors::AppError;
use sea_orm::DatabaseConnection;
use std::sync::Arc;

pub struct AccountingService {
    db: Arc<DatabaseConnection>,
}

impl AccountingService {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }

    /// Record a transaction in the accounting ledger. Currently this
    /// only logs the operation.
    pub async fn record_transaction(
        &self,
        description: &str,
        amount: rust_decimal::Decimal,
    ) -> Result<(), AppError> {
        tracing::info!(description, %amount, "record accounting transaction");
        // TODO: insert ledger entry into database
        Ok(())
    }
}
