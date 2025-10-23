use sea_orm::DatabaseConnection;
use std::sync::Arc;

pub struct ForecastingService {
    db: Arc<DatabaseConnection>,
}

impl ForecastingService {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }
}
