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
}