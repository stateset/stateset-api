use sea_orm::{Database, DatabaseConnection, DbErr, ConnectOptions};
use sea_orm_migration::MigratorTrait;
use crate::errors::AppError;
use std::time::Duration;
use anyhow::Context;

/// Type alias for a database connection pool
pub type DbPool = DatabaseConnection;

/// Configuration for database connection
#[derive(Debug, Clone)]
pub struct DbConfig {
    /// Database connection URL
    pub url: String,
    /// Maximum number of connections
    pub max_connections: u32,
    /// Minimum number of connections
    pub min_connections: u32,
    /// Connection timeout duration
    pub connect_timeout: Duration,
    /// Idle timeout duration
    pub idle_timeout: Duration,
}

impl Default for DbConfig {
    fn default() -> Self {
        Self {
            url: String::new(),
            max_connections: 10,
            min_connections: 1,
            connect_timeout: Duration::from_secs(30),
            idle_timeout: Duration::from_secs(600),
        }
    }
}

/// Establishes a connection pool to the database
///
/// # Arguments
/// * `database_url` - Database connection URL string
///
/// # Errors
/// Returns an `AppError` if the connection cannot be established
pub async fn establish_connection(database_url: &str) -> Result<DbPool, AppError> {
    let mut opt = ConnectOptions::new(database_url);
    opt.max_connections(10)
       .min_connections(1)
       .connect_timeout(Duration::from_secs(30))
       .idle_timeout(Duration::from_secs(600))
       .sqlx_logging(true)
       .to_owned();

    Database::connect(opt)
        .await
        .map_err(|e| AppError::DatabaseError(format!("Failed to connect to database: {}", e)))
        .context("Database connection establishment failed")
}

/// Simple function to create a database connection
/// 
/// # Arguments
/// * `database_url` - Database connection URL string
///
/// # Errors
/// Returns an error if the connection cannot be established
pub async fn connect(database_url: &str) -> Result<DbPool, anyhow::Error> {
    establish_connection(database_url).await.map_err(Into::into)
}

/// Establishes a connection pool to the database with custom configuration
///
/// # Arguments
/// * `config` - Database configuration settings
///
/// # Errors
/// Returns an `AppError` if the connection cannot be established
pub async fn establish_connection_with_config(config: &DbConfig) -> Result<DbPool, AppError> {
    let opt = ConnectOptions::new(&config.url)
        .max_connections(config.max_connections)
        .min_connections(config.min_connections)
        .connect_timeout(config.connect_timeout)
        .idle_timeout(config.idle_timeout)
        .sqlx_logging(true)
        .to_owned();

    Database::connect(opt)
        .await
        .map_err(|e| AppError::DatabaseError(format!("Failed to connect to database: {}", e)))
        .context("Database connection establishment failed")
}

/// Runs database migrations
///
/// # Arguments
/// * `pool` - Reference to the database connection pool
///
/// # Errors
/// Returns an `AppError` if migrations fail to execute
pub async fn run_migrations(pool: &DbPool) -> Result<(), AppError> {
    crate::migrator::Migrator::up(pool, None)
        .await
        .map_err(|e| AppError::MigrationError(format!("Migration failed: {}", e)))
        .context("Database migration execution failed")
}

/// Checks if the database connection is active
pub async fn check_connection(pool: &DbPool) -> Result<(), AppError> {
    pool.ping()
        .await
        .map_err(|e| AppError::DatabaseError(format!("Connection check failed: {}", e)))
}

/// Trait for async database operations with better error handling
#[async_trait::async_trait]
pub trait AsyncDatabase {
    /// Executes a database operation asynchronously
    ///
    /// # Arguments
    /// * `f` - Closure containing the database operation
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
        tokio::task::spawn_blocking(move || {
            f(&pool).map_err(|e| AppError::DatabaseError(e.to_string()))
        })
        .await
        .context("Async database operation failed")?
    }
}

/// Closes the database connection pool
pub async fn close_pool(pool: DbPool) -> Result<(), AppError> {
    pool.close()
        .await
        .map_err(|e| AppError::DatabaseError(format!("Failed to close pool: {}", e)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    async fn setup_test_pool() -> Result<DbPool, AppError> {
        let database_url = env::var("DATABASE_URL")
            .expect("DATABASE_URL must be set for tests");
        
        establish_connection(&database_url).await
    }

    #[tokio::test]
    async fn test_establish_connection() {
        let pool = setup_test_pool().await.expect("Failed to establish connection");
        assert!(check_connection(&pool).await.is_ok());
    }

    #[tokio::test]
    async fn test_run_migrations() {
        let pool = setup_test_pool().await.expect("Failed to establish connection");
        assert!(run_migrations(&pool).await.is_ok());
    }

    #[tokio::test]
    async fn test_async_execute() {
        let pool = setup_test_pool().await.expect("Failed to establish connection");
        
        let result = pool.execute_async(|conn| {
            // Example operation - replace with actual test query
            Ok(42)
        }).await;
        
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
    }
}