use crate::errors::AppError;
use sea_orm::DatabaseConnection;
use std::sync::Arc;

pub struct AccountService {
    db: Arc<DatabaseConnection>,
}

impl AccountService {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }
}
