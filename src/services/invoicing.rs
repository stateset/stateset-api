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
}