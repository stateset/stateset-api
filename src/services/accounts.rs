use crate::errors::AppError;
use std::sync::Arc;
use sea_orm::DatabaseConnection;

pub struct AccountService {
    db: Arc<DatabaseConnection>,
}

impl AccountService {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }
}