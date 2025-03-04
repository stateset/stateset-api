use crate::errors::AppError;
use std::sync::Arc;
use sea_orm::DatabaseConnection;

pub struct ForecastingService {
    db: Arc<DatabaseConnection>,
}

impl ForecastingService {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }
}