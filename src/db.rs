use sea_orm::{Database, DatabaseConnection, DbErr};
use sea_orm_migration::MigratorTrait;
use crate::errors::AppError;

/// Type alias for a database connection pool
pub type DbPool = DatabaseConnection;

/// Establishes a connection pool to the database
pub async fn establish_connection(database_url: &str) -> Result<DbPool, AppError> {
    Database::connect(database_url)
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))
}

/// Runs database migrations
pub async fn run_migrations(pool: &DbPool) -> Result<(), AppError> {
    let migrator = crate::migrator::Migrator; // Ensure you have a migrator module configured
    migrator
        .up(pool, None)
        .await
        .map_err(|e| AppError::MigrationError(e.to_string()))
}

/// Provides a connection from the pool
pub fn get_connection(pool: &DbPool) -> Result<&DbPool, AppError> {
    Ok(pool)
}

/// Trait for adding async database operations
#[async_trait::async_trait]
pub trait AsyncDatabase {
    async fn execute_async<F, T>(&self, f: F) -> Result<T, AppError>
    where
        F: FnOnce(&DbPool) -> Result<T, DbErr> + Send + 'static,
        T: Send + 'static;
}

#[async_trait::async_trait]
impl AsyncDatabase for DbPool {
    async fn execute_async<F, T>(&self, f: F) -> Result<T, AppError>
    where
        F: FnOnce(&DbPool) -> Result<T, DbErr> + Send + 'static,
        T: Send + 'static,
    {
        let pool = self.clone();
        tokio::task::spawn_blocking(move || f(&pool))
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?
            .map_err(|e| AppError::DatabaseError(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::runtime::Runtime;

    #[test]
    fn test_establish_connection() {
        let rt = Runtime::new().unwrap();
        let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");

        rt.block_on(async {
            let pool = establish_connection(&database_url).await.expect("Failed to establish connection");
            assert!(pool.is_active().await);
        });
    }

    #[test]
    fn test_run_migrations() {
        let rt = Runtime::new().unwrap();
        let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");

        rt.block_on(async {
            let pool = establish_connection(&database_url).await.expect("Failed to establish connection");
            assert!(run_migrations(&pool).await.is_ok());
        });
    }
}
