pub mod query_builder;

use crate::config::AppConfig;
use crate::errors::{AppError, ServiceError};
use anyhow::Context;
use futures::future::BoxFuture;
use metrics::{counter, gauge, histogram};
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DatabaseTransaction, DbBackend,
    DbErr, FromQueryResult, Statement, TransactionTrait,
};
use sea_orm_migration::MigratorTrait;
use std::future::Future;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

pub use query_builder::{QueryBuilder, SearchBuilder};

/// Type alias for a database connection pool
pub type DbPool = DatabaseConnection;

/// Database metrics tracking prefix
const METRICS_PREFIX: &str = "stateset_db";

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
    /// Acquire connection timeout
    pub acquire_timeout: Duration,
    /// Statement timeout
    pub statement_timeout: Option<Duration>,
}

impl Default for DbConfig {
    fn default() -> Self {
        Self {
            url: String::new(),
            max_connections: 10,
            min_connections: 1,
            connect_timeout: Duration::from_secs(30),
            idle_timeout: Duration::from_secs(600),
            acquire_timeout: Duration::from_secs(8),
            statement_timeout: Some(Duration::from_secs(30)),
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
    let config = DbConfig {
        url: database_url.to_string(),
        ..Default::default()
    };

    establish_connection_with_config(&config).await
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
    debug!("Configuring database connection with: {:?}", config);

    let mut opt = ConnectOptions::new(config.url.clone());

    opt.max_connections(config.max_connections)
        .min_connections(config.min_connections)
        .connect_timeout(config.connect_timeout)
        .acquire_timeout(config.acquire_timeout)
        .idle_timeout(config.idle_timeout)
        .sqlx_logging(true);

    if let Some(timeout) = config.statement_timeout {
        // TODO: Fix statement timeout API
        // opt.set_statement_timeout(Some(timeout));
    }

    // Register metrics
    gauge!("stateset_db.max_connections", config.max_connections as f64);

    info!(
        "Connecting to database with max_connections={}",
        config.max_connections
    );

    // Connect and initialize connection pool
    let db_pool = Database::connect(opt)
        .await
        .map_err(|e| AppError::DatabaseError(e))
        .context("Database connection establishment failed")?;

    info!("Database connection pool established successfully");

    Ok(db_pool)
}

impl From<&AppConfig> for DbConfig {
    fn from(cfg: &AppConfig) -> Self {
        Self {
            url: cfg.database_url.clone(),
            max_connections: cfg.db_max_connections,
            min_connections: cfg.db_min_connections,
            connect_timeout: Duration::from_secs(cfg.db_connect_timeout_secs),
            idle_timeout: Duration::from_secs(cfg.db_idle_timeout_secs),
            acquire_timeout: Duration::from_secs(cfg.db_acquire_timeout_secs),
            statement_timeout: cfg.db_statement_timeout_secs.map(Duration::from_secs),
        }
    }
}

/// Establish DB pool using AppConfig tuning
pub async fn establish_connection_from_app_config(cfg: &AppConfig) -> Result<DbPool, AppError> {
    let db_cfg: DbConfig = cfg.into();
    establish_connection_with_config(&db_cfg).await
}

/// Convenience helper to create a DB pool using loaded AppConfig
pub async fn create_db_pool() -> Result<DbPool, AppError> {
    let cfg = crate::config::load_config()
        .map_err(|e| ServiceError::InternalError(format!("Failed to load config: {}", e)))?;
    establish_connection_from_app_config(&cfg).await
}

/// Database access wrapper with built-in metrics and error handling
#[derive(Debug, Clone)]
pub struct DatabaseAccess {
    pool: Arc<DbPool>,
}

impl DatabaseAccess {
    /// Create a new database access instance
    pub fn new(pool: Arc<DbPool>) -> Self {
        Self { pool }
    }

    /// Get a reference to the connection pool
    pub fn get_pool(&self) -> &DbPool {
        &self.pool
    }

    /// Execute raw SQL with parameters
    pub async fn execute_raw<T>(
        &self,
        sql: &str,
        params: Vec<sea_orm::Value>,
    ) -> Result<T, ServiceError>
    where
        T: FromQueryResult + Send + Sync,
    {
        let db = &*self.pool;
        let start = std::time::Instant::now();
        let stmt = Statement::from_sql_and_values(DbBackend::Postgres, sql, params);

        debug!("Executing SQL query: {:?}", stmt);

        let result = db
            .query_one(stmt)
            .await
            .map_err(|e| {
                error!("Database error executing raw SQL: {}", e);
                counter!("stateset_db.query.error", 1);
                ServiceError::db_error(e)
            })?
            .ok_or_else(|| ServiceError::NotFoundError("No data found".to_string()))?;

        let elapsed = start.elapsed();
        histogram!("stateset_db.query.duration", elapsed);
        debug!("Raw SQL query completed in {:?}", elapsed);

        T::from_query_result(&result, "").map_err(|e| {
            error!("Failed to convert query result: {}", e);
            ServiceError::db_error(e)
        })
    }

    /// Start a transaction
    pub async fn transaction<F, T, E>(&self, f: F) -> Result<T, E>
    where
        F: for<'a> FnOnce(&'a DatabaseTransaction) -> BoxFuture<'a, Result<T, E>> + Send,
        T: Send + 'static,
        E: From<DbErr> + Send + 'static + std::error::Error,
    {
        let db = &*self.pool;
        let transaction_id = Uuid::new_v4();
        let start = std::time::Instant::now();

        debug!(transaction_id = %transaction_id, "Starting database transaction");
        counter!("stateset_db.transaction.started", 1);

        let result = db
            .transaction(move |txn| {
                let future = f(txn);
                Box::pin(async move {
                    let result = future.await;
                    debug!(transaction_id = %transaction_id, "Transaction completed");
                    result
                })
            })
            .await;

        let elapsed = start.elapsed();
        histogram!("stateset_db.transaction.duration", elapsed);

        match &result {
            Ok(_) => {
                counter!("stateset_db.transaction.committed", 1);
                debug!(transaction_id = %transaction_id, "Transaction committed successfully in {:?}", elapsed);
            }
            Err(_) => {
                counter!("stateset_db.transaction.rolled_back", 1);
                warn!(transaction_id = %transaction_id, "Transaction rolled back after {:?}", elapsed);
            }
        }

        result.map_err(|e| match e {
            sea_orm::TransactionError::Connection(e) => E::from(e),
            sea_orm::TransactionError::Transaction(e) => e,
        })
    }

    /// Execute query with metrics and logging
    pub async fn execute<F, Fut, T>(&self, operation: &str, f: F) -> Result<T, ServiceError>
    where
        F: FnOnce(&DbPool) -> Fut + Send,
        Fut: Future<Output = Result<T, DbErr>> + Send,
        T: Send + 'static,
    {
        let db = &*self.pool;
        let start = std::time::Instant::now();

        debug!(operation = %operation, "Starting database operation");

        let result = f(db).await.map_err(|e| {
            error!(operation = %operation, error = %e, "Database operation failed");
            counter!("stateset_db.operation.error", 1, "operation" => operation.to_string());
            ServiceError::db_error(e)
        });

        let elapsed = start.elapsed();
        histogram!("stateset_db.operation.duration", elapsed, "operation" => operation.to_string());

        if result.is_ok() {
            debug!(operation = %operation, duration = ?elapsed, "Database operation completed successfully");
        }

        result
    }

    /// Find entity by ID with metrics
    // TODO: Fix generic constraints for find_by_id
    /*
    pub async fn find_by_id<E: EntityTrait>(
        &self,
        id: E::PrimaryKey,
    ) -> Result<Option<E::Model>, ServiceError>
    where
        E::Model: Send + Sync,
    {
        let entity_name = std::any::type_name::<E>()
            .split("::")
            .last()
            .unwrap_or("Unknown");
        self.execute(&format!("find_by_id:{}", entity_name), |db| {
            E::find_by_id(id).one(db)
        })
        .await
    }
    */

    /// Get the underlying connection - helper for compatibility
    pub async fn get(&self) -> Result<&DatabaseConnection, ServiceError> {
        // This is a compatibility method for code that uses pool.get() pattern
        Ok(&self.pool)
    }
}

/// Runs database migrations
///
/// # Arguments
/// * `pool` - Reference to the database connection pool
///
/// # Errors
/// Returns an `AppError` if migrations fail to execute
pub async fn run_migrations(pool: &DbPool) -> Result<(), AppError> {
    info!("Running database migrations");
    let start = std::time::Instant::now();

    // Execute migrations using our embedded migrator
    let result = crate::migrator::Migrator::up(pool, None)
        .await
        .map_err(AppError::DatabaseError);

    let elapsed = start.elapsed();
    match &result {
        Ok(_) => info!(
            "Database migrations completed successfully in {:?}",
            elapsed
        ),
        Err(e) => error!("Database migrations failed after {:?}: {}", elapsed, e),
    }

    result
}

/// Checks if the database connection is active
pub async fn check_connection(pool: &DbPool) -> Result<(), AppError> {
    debug!("Checking database connection");
    let start = std::time::Instant::now();

    let result = pool.ping().await.map_err(|e| AppError::DatabaseError(e));

    let elapsed = start.elapsed();
    match &result {
        Ok(_) => {
            debug!("Database connection check successful in {:?}", elapsed);
            gauge!("stateset_db.connection_latency", elapsed.as_millis() as f64);
        }
        Err(e) => {
            error!(
                "Database connection check failed after {:?}: {}",
                elapsed, e
            );
            counter!("stateset_db.connection_failures", 1);
        }
    }

    result
}

/// Closes the database connection pool
pub async fn close_pool(pool: DbPool) -> Result<(), AppError> {
    info!("Closing database connection pool");

    pool.close().await.map_err(|e| AppError::DatabaseError(e))
}

#[cfg(all(test, feature = "mock-tests"))]
mod tests {
    use super::*;
    use std::env;

    async fn setup_test_pool() -> Result<DbPool, AppError> {
        let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set for tests");

        establish_connection(&database_url).await
    }

    #[tokio::test]
    async fn test_establish_connection() {
        let pool = setup_test_pool()
            .await
            .expect("Failed to establish connection");
        assert!(check_connection(&pool).await.is_ok());
    }

    #[tokio::test]
    async fn test_run_migrations() {
        let pool = setup_test_pool()
            .await
            .expect("Failed to establish connection");
        assert!(run_migrations(&pool).await.is_ok());
    }

    #[tokio::test]
    async fn test_database_access_transaction() {
        let pool = setup_test_pool()
            .await
            .expect("Failed to establish connection");
        let db_access = DatabaseAccess::new(Arc::new(pool));

        let result = db_access
            .transaction(|txn| {
                Box::pin(async move {
                    // Simple test query
                    Ok::<_, DbErr>(42)
                })
            })
            .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
    }
}
