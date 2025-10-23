use sea_orm::DatabaseConnection;
use std::sync::Arc;

pub struct LeadService {
    db: Arc<DatabaseConnection>,
}

impl LeadService {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }
}
