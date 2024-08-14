use diesel::prelude::*;
use diesel::r2d2::{self, ConnectionManager};
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use tokio_diesel::AsyncRunQueryDsl;

use crate::errors::AppError;

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!();

/// Type alias for a database connection pool
pub type DbPool = r2d2::Pool<ConnectionManager<PgConnection>>;

/// Establishes a connection pool to the database
pub fn establish_connection(database_url: &str) -> Result<DbPool, AppError> {
    let manager = ConnectionManager::<PgConnection>::new(database_url);
    r2d2::Pool::builder()
        .build(manager)
        .map_err(|e| AppError::DatabaseError(e.to_string()))
}

/// Runs database migrations
pub fn run_migrations(pool: &DbPool) -> Result<(), AppError> {
    let mut conn = pool.get().map_err(|e| AppError::DatabaseError(e.to_string()))?;
    conn.run_pending_migrations(MIGRATIONS)
        .map_err(|e| AppError::MigrationError(e.to_string()))?;
    Ok(())
}

/// Provides a connection from the pool
pub fn get_connection(pool: &DbPool) -> Result<r2d2::PooledConnection<ConnectionManager<PgConnection>>, AppError> {
    pool.get().map_err(|e| AppError::DatabaseError(e.to_string()))
}

/// Trait for adding async database operations
#[async_trait::async_trait]
pub trait AsyncDatabase {
    async fn execute_async<F, T>(&self, f: F) -> Result<T, AppError>
    where
        F: FnOnce(&PgConnection) -> Result<T, diesel::result::Error> + Send + 'static,
        T: Send + 'static;
}

#[async_trait::async_trait]
impl AsyncDatabase for DbPool {
    async fn execute_async<F, T>(&self, f: F) -> Result<T, AppError>
    where
        F: FnOnce(&PgConnection) -> Result<T, diesel::result::Error> + Send + 'static,
        T: Send + 'static,
    {
        let pool = self.clone();
        tokio::task::spawn_blocking(move || {
            let conn = pool.get()?;
            f(&conn)
        })
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?
        .map_err(|e| AppError::DatabaseError(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_establish_connection() {
        let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
        let pool = establish_connection(&database_url).expect("Failed to establish connection");
        assert!(pool.get().is_ok());
    }

    #[test]
    fn test_run_migrations() {
        let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
        let pool = establish_connection(&database_url).expect("Failed to establish connection");
        assert!(run_migrations(&pool).is_ok());
    }
}