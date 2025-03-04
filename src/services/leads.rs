use crate::errors::AppError;
use std::sync::Arc;
use sea_orm::DatabaseConnection;

pub struct LeadService {
    db: Arc<DatabaseConnection>,
}

impl LeadService {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }
}