use sea_orm::DatabaseConnection;
use std::sync::Arc;

pub mod order_repository;

/// Repository trait for common database operations
pub trait Repository {
    fn get_db(&self) -> &DatabaseConnection;
}

#[derive(Debug)]
pub struct BaseRepository {
    db: Arc<DatabaseConnection>,
}

impl BaseRepository {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }
}

impl Repository for BaseRepository {
    fn get_db(&self) -> &DatabaseConnection {
        &self.db
    }
}
