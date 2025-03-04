use crate::errors::AppError;
use std::sync::Arc;
use sea_orm::DatabaseConnection;

pub struct AccountingService {
    db: Arc<DatabaseConnection>,
}

impl AccountingService {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }
}