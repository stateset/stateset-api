use crate::errors::AppError;
use std::sync::Arc;
use sea_orm::DatabaseConnection;

pub struct BusinessIntelligenceService {
    db: Arc<DatabaseConnection>,
}

impl BusinessIntelligenceService {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }
}