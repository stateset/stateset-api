use crate::errors::AppError;
use sea_orm::DatabaseConnection;
use std::sync::Arc;

pub struct BusinessIntelligenceService {
    db: Arc<DatabaseConnection>,
}

impl BusinessIntelligenceService {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }
}
