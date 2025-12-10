pub mod query_builder;

use crate::circuit_breaker::{CircuitBreaker, CircuitBreakerConfig, CircuitState};
use crate::config::AppConfig;
use crate::errors::{AppError, ServiceError};
use anyhow::Context;
use futures::future::BoxFuture;
use metrics::{counter, gauge, histogram};
use sea_orm::sea_query::TableCreateStatement;
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DatabaseTransaction, DbBackend,
    DbErr, FromQueryResult, Schema, Statement, TransactionTrait,
};
use sea_orm_migration::MigratorTrait;
use std::future::Future;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Configuration for database retry logic
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Maximum number of retry attempts
    pub max_retries: u32,
    /// Initial delay between retries
    pub initial_delay: Duration,
    /// Maximum delay between retries
    pub max_delay: Duration,
    /// Multiplier for exponential backoff
    pub backoff_multiplier: f64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(5),
            backoff_multiplier: 2.0,
        }
    }
}

/// Determines if an error is retryable (transient)
fn is_retryable_error(err: &DbErr) -> bool {
    match err {
        DbErr::Conn(_) => true, // Connection errors are retryable
        DbErr::ConnectionAcquire(_) => true, // Pool exhaustion is retryable
        DbErr::Query(ref runtime_err) => {
            let msg = runtime_err.to_string().to_lowercase();
            // Retry on connection-related query errors
            msg.contains("connection")
                || msg.contains("timeout")
                || msg.contains("broken pipe")
                || msg.contains("reset by peer")
                || msg.contains("deadlock")
        }
        _ => false,
    }
}

/// Execute a database operation with retry logic and exponential backoff
pub async fn with_retry<F, Fut, T>(
    config: &RetryConfig,
    operation_name: &str,
    mut f: F,
) -> Result<T, DbErr>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, DbErr>>,
{
    let mut attempts = 0;
    let mut delay = config.initial_delay;

    loop {
        attempts += 1;

        match f().await {
            Ok(result) => {
                if attempts > 1 {
                    info!(
                        operation = %operation_name,
                        attempts = attempts,
                        "Database operation succeeded after {} attempts",
                        attempts
                    );
                    counter!("stateset_db.retry.success", 1, "operation" => operation_name.to_string());
                }
                return Ok(result);
            }
            Err(err) => {
                if attempts >= config.max_retries || !is_retryable_error(&err) {
                    error!(
                        operation = %operation_name,
                        attempts = attempts,
                        error = %err,
                        "Database operation failed after {} attempts (non-retryable or max retries reached)",
                        attempts
                    );
                    counter!("stateset_db.retry.exhausted", 1, "operation" => operation_name.to_string());
                    return Err(err);
                }

                warn!(
                    operation = %operation_name,
                    attempts = attempts,
                    max_retries = config.max_retries,
                    delay_ms = delay.as_millis() as u64,
                    error = %err,
                    "Retryable database error, waiting {:?} before retry",
                    delay
                );
                counter!("stateset_db.retry.attempt", 1, "operation" => operation_name.to_string());

                sleep(delay).await;

                // Exponential backoff with max cap
                delay = Duration::from_secs_f64(
                    (delay.as_secs_f64() * config.backoff_multiplier).min(config.max_delay.as_secs_f64())
                );
            }
        }
    }
}

use crate::auth::{api_key, refresh_token, user, user_role};
use crate::entities::commerce::{
    customer as commerce_customer, customer_address as commerce_customer_address,
    product_variant as commerce_product_variant,
};
use crate::entities::{
    inventory_balance, inventory_location, inventory_transaction, item_master, order,
    order_fulfillments, order_item, po_receipt_headers, po_receipt_lines, product,
    purchase_order_headers, purchase_order_lines, sales_order_header, sales_order_line, shipment,
};

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

    // Set statement timeout using raw SQL for PostgreSQL
    if let Some(timeout) = config.statement_timeout {
        let backend = db_pool.get_database_backend();
        if backend == DbBackend::Postgres {
            let timeout_ms = timeout.as_millis() as i64;
            let sql = format!("SET statement_timeout = {}", timeout_ms);
            match db_pool.execute(Statement::from_string(backend, sql)).await {
                Ok(_) => info!("Statement timeout set to {}ms", timeout_ms),
                Err(e) => warn!("Failed to set statement timeout: {}", e),
            }
        }
    }

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

/// Database access wrapper with built-in metrics, error handling, and retry logic
#[derive(Debug, Clone)]
pub struct DatabaseAccess {
    pool: Arc<DbPool>,
    retry_config: RetryConfig,
}

impl DatabaseAccess {
    /// Create a new database access instance with default retry config
    pub fn new(pool: Arc<DbPool>) -> Self {
        Self {
            pool,
            retry_config: RetryConfig::default(),
        }
    }

    /// Create a new database access instance with custom retry config
    pub fn with_retry_config(pool: Arc<DbPool>, retry_config: RetryConfig) -> Self {
        Self { pool, retry_config }
    }

    /// Get the retry configuration
    pub fn retry_config(&self) -> &RetryConfig {
        &self.retry_config
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
            .ok_or_else(|| ServiceError::NotFound("No data found".to_string()))?;

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

    /// Execute query with retry logic for transient failures
    ///
    /// This method will automatically retry on connection errors, timeouts,
    /// and other transient database failures using exponential backoff.
    pub async fn execute_with_retry<F, Fut, T>(
        &self,
        operation: &str,
        f: F,
    ) -> Result<T, ServiceError>
    where
        F: Fn() -> Fut + Send,
        Fut: Future<Output = Result<T, DbErr>> + Send,
        T: Send + 'static,
    {
        let start = std::time::Instant::now();

        debug!(operation = %operation, "Starting database operation with retry");

        let result = with_retry(&self.retry_config, operation, f)
            .await
            .map_err(|e| {
                error!(operation = %operation, error = %e, "Database operation failed after retries");
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

    /// Check if the database connection is healthy
    pub async fn health_check(&self) -> Result<(), ServiceError> {
        let db = &*self.pool;
        let start = std::time::Instant::now();

        debug!("Performing database health check");

        let result = db.ping().await.map_err(|e| {
            error!(error = %e, "Database health check failed");
            counter!("stateset_db.health_check.failed", 1);
            ServiceError::db_error(e)
        });

        let elapsed = start.elapsed();
        gauge!("stateset_db.health_check.latency_ms", elapsed.as_millis() as f64);

        if result.is_ok() {
            debug!(latency_ms = elapsed.as_millis() as u64, "Database health check passed");
            counter!("stateset_db.health_check.passed", 1);
        }

        result
    }

    /// Find entity by ID with metrics
    ///
    /// Note: Use entity-specific repository methods for type-safe queries.
    /// Generic find_by_id requires additional trait bounds that vary per entity.

    /// Get the underlying connection - helper for compatibility
    pub async fn get(&self) -> Result<&DatabaseConnection, ServiceError> {
        // This is a compatibility method for code that uses pool.get() pattern
        Ok(&self.pool)
    }
}

/// Configuration for database circuit breaker
#[derive(Debug, Clone)]
pub struct DbCircuitBreakerConfig {
    /// Number of failures before opening the circuit
    pub failure_threshold: u32,
    /// Duration to wait before transitioning from Open to HalfOpen
    pub timeout: Duration,
    /// Number of successful requests needed in HalfOpen to close the circuit
    pub success_threshold: u32,
}

impl Default for DbCircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            timeout: Duration::from_secs(30),
            success_threshold: 2,
        }
    }
}

/// Database access wrapper with circuit breaker, retry logic, and metrics
///
/// This provides protection against cascading failures when the database
/// is experiencing issues. When failures exceed the threshold, the circuit
/// opens and requests fail fast rather than waiting for timeouts.
#[derive(Clone)]
pub struct ResilientDatabaseAccess {
    pool: Arc<DbPool>,
    retry_config: RetryConfig,
    circuit_breaker: Arc<CircuitBreaker>,
}

impl ResilientDatabaseAccess {
    /// Create a new resilient database access instance with default configurations
    pub fn new(pool: Arc<DbPool>) -> Self {
        let cb_config = CircuitBreakerConfig::default();
        Self {
            pool,
            retry_config: RetryConfig::default(),
            circuit_breaker: Arc::new(CircuitBreaker::with_config(cb_config)),
        }
    }

    /// Create a new instance with custom retry and circuit breaker configurations
    pub fn with_config(
        pool: Arc<DbPool>,
        retry_config: RetryConfig,
        cb_config: DbCircuitBreakerConfig,
    ) -> Self {
        let circuit_config = CircuitBreakerConfig {
            failure_threshold: cb_config.failure_threshold,
            timeout: cb_config.timeout,
            success_threshold: cb_config.success_threshold,
        };
        Self {
            pool,
            retry_config,
            circuit_breaker: Arc::new(CircuitBreaker::with_config(circuit_config)),
        }
    }

    /// Get the current circuit breaker state
    pub fn circuit_state(&self) -> CircuitState {
        self.circuit_breaker.state()
    }

    /// Check if the circuit is allowing requests
    pub fn is_circuit_closed(&self) -> bool {
        matches!(self.circuit_breaker.state(), CircuitState::Closed)
    }

    /// Get a reference to the connection pool
    pub fn get_pool(&self) -> &DbPool {
        &self.pool
    }

    /// Execute a database operation with circuit breaker protection
    ///
    /// If the circuit is open, this will fail fast with an error instead
    /// of attempting the operation. This prevents overwhelming an already
    /// struggling database with more requests.
    pub async fn execute<F, Fut, T>(&self, operation: &str, f: F) -> Result<T, ServiceError>
    where
        F: FnOnce(&DbPool) -> Fut + Send,
        Fut: Future<Output = Result<T, DbErr>> + Send,
        T: Send + 'static,
    {
        let state = self.circuit_breaker.state();

        // Check circuit state first
        match state {
            CircuitState::Open => {
                counter!("stateset_db.circuit_breaker.rejected", 1, "operation" => operation.to_string());
                warn!(
                    operation = %operation,
                    "Database circuit breaker is open, rejecting request"
                );
                return Err(ServiceError::ServiceUnavailable(
                    "Database service temporarily unavailable (circuit breaker open)".to_string(),
                ));
            }
            CircuitState::HalfOpen => {
                debug!(
                    operation = %operation,
                    "Database circuit breaker is half-open, allowing probe request"
                );
            }
            CircuitState::Closed => {}
        }

        let db = &*self.pool;
        let start = std::time::Instant::now();

        debug!(operation = %operation, "Starting database operation with circuit breaker protection");

        let result = f(db).await;
        let elapsed = start.elapsed();

        match result {
            Ok(value) => {
                self.circuit_breaker.on_success();
                histogram!("stateset_db.operation.duration", elapsed, "operation" => operation.to_string());
                counter!("stateset_db.operation.success", 1, "operation" => operation.to_string());
                debug!(operation = %operation, duration = ?elapsed, "Database operation completed successfully");
                Ok(value)
            }
            Err(e) => {
                // Only trip circuit on connection-related errors
                if is_retryable_error(&e) {
                    self.circuit_breaker.on_failure();
                    counter!("stateset_db.circuit_breaker.failure_recorded", 1, "operation" => operation.to_string());
                }
                error!(operation = %operation, error = %e, "Database operation failed");
                counter!("stateset_db.operation.error", 1, "operation" => operation.to_string());
                Err(ServiceError::DatabaseError(e))
            }
        }
    }

    /// Execute a database operation with both circuit breaker and retry logic
    ///
    /// This combines circuit breaker protection with automatic retries for
    /// transient failures. Retries only occur within the context of the
    /// circuit breaker - if the circuit opens, no more retries will be attempted.
    pub async fn execute_with_retry<F, Fut, T>(
        &self,
        operation: &str,
        f: F,
    ) -> Result<T, ServiceError>
    where
        F: Fn() -> Fut + Send,
        Fut: Future<Output = Result<T, DbErr>> + Send,
        T: Send + 'static,
    {
        // Check circuit state first
        if matches!(self.circuit_breaker.state(), CircuitState::Open) {
            counter!("stateset_db.circuit_breaker.rejected", 1, "operation" => operation.to_string());
            warn!(
                operation = %operation,
                "Database circuit breaker is open, rejecting request"
            );
            return Err(ServiceError::ServiceUnavailable(
                "Database service temporarily unavailable (circuit breaker open)".to_string(),
            ));
        }

        let start = std::time::Instant::now();
        let mut attempts = 0;
        let mut delay = self.retry_config.initial_delay;

        loop {
            attempts += 1;

            // Re-check circuit state before each attempt
            if attempts > 1 && matches!(self.circuit_breaker.state(), CircuitState::Open) {
                warn!(
                    operation = %operation,
                    attempts = attempts,
                    "Circuit opened during retry attempts, aborting"
                );
                return Err(ServiceError::ServiceUnavailable(
                    "Database circuit breaker opened during operation".to_string(),
                ));
            }

            match f().await {
                Ok(result) => {
                    self.circuit_breaker.on_success();
                    let elapsed = start.elapsed();
                    histogram!("stateset_db.operation.duration", elapsed, "operation" => operation.to_string());

                    if attempts > 1 {
                        info!(
                            operation = %operation,
                            attempts = attempts,
                            "Database operation succeeded after {} attempts",
                            attempts
                        );
                        counter!("stateset_db.retry.success", 1, "operation" => operation.to_string());
                    }
                    return Ok(result);
                }
                Err(err) => {
                    let is_retryable = is_retryable_error(&err);

                    if is_retryable {
                        self.circuit_breaker.on_failure();
                    }

                    if attempts >= self.retry_config.max_retries || !is_retryable {
                        error!(
                            operation = %operation,
                            attempts = attempts,
                            error = %err,
                            "Database operation failed after {} attempts",
                            attempts
                        );
                        counter!("stateset_db.retry.exhausted", 1, "operation" => operation.to_string());
                        return Err(ServiceError::db_error(err));
                    }

                    warn!(
                        operation = %operation,
                        attempts = attempts,
                        max_retries = self.retry_config.max_retries,
                        delay_ms = delay.as_millis() as u64,
                        error = %err,
                        "Retryable database error, waiting {:?} before retry",
                        delay
                    );
                    counter!("stateset_db.retry.attempt", 1, "operation" => operation.to_string());

                    sleep(delay).await;

                    // Exponential backoff with max cap
                    delay = Duration::from_secs_f64(
                        (delay.as_secs_f64() * self.retry_config.backoff_multiplier)
                            .min(self.retry_config.max_delay.as_secs_f64()),
                    );
                }
            }
        }
    }

    /// Check if the database connection is healthy
    pub async fn health_check(&self) -> Result<(), ServiceError> {
        let db = &*self.pool;
        let start = std::time::Instant::now();

        debug!("Performing database health check");

        let result = db.ping().await;
        let elapsed = start.elapsed();
        gauge!("stateset_db.health_check.latency_ms", elapsed.as_millis() as f64);

        match result {
            Ok(_) => {
                self.circuit_breaker.on_success();
                debug!(latency_ms = elapsed.as_millis() as u64, "Database health check passed");
                counter!("stateset_db.health_check.passed", 1);
                Ok(())
            }
            Err(e) => {
                self.circuit_breaker.on_failure();
                error!(error = %e, "Database health check failed");
                counter!("stateset_db.health_check.failed", 1);
                Err(ServiceError::db_error(e))
            }
        }
    }

    /// Get circuit breaker metrics for monitoring
    pub fn circuit_breaker_metrics(&self) -> crate::circuit_breaker::CircuitBreakerMetrics {
        self.circuit_breaker.metrics()
    }
}

impl std::fmt::Debug for ResilientDatabaseAccess {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ResilientDatabaseAccess")
            .field("circuit_state", &self.circuit_breaker.state())
            .field("retry_config", &self.retry_config)
            .finish()
    }
}

async fn ensure_core_tables(pool: &DbPool) -> Result<(), AppError> {
    let backend = pool.get_database_backend();
    debug!(backend = ?backend, "Ensuring core tables via entity schema");

    if matches!(backend, DbBackend::Sqlite) {
        debug!("Skipping core table auto-creation for SQLite backend; relying on migrations");
        return Ok(());
    }

    let schema = Schema::new(backend);
    let mut tables = core_table_definitions(&schema, backend);

    for (name, mut table) in tables.drain(..) {
        table.if_not_exists();
        let statement = backend.build(&table);
        if let Err(err) = pool.execute(statement).await {
            warn!(
                table = name,
                "Failed to ensure existence of table `{}`: {}", name, err
            );
            return Err(AppError::DatabaseError(err));
        }
    }

    Ok(())
}

fn core_table_definitions(
    schema: &Schema,
    backend: DbBackend,
) -> Vec<(&'static str, TableCreateStatement)> {
    let mut tables: Vec<(&'static str, TableCreateStatement)> = vec![
        (
            "item_master",
            schema.create_table_from_entity(item_master::Entity),
        ),
        (
            "inventory_locations",
            schema.create_table_from_entity(inventory_location::Entity),
        ),
        (
            "inventory_balances",
            schema.create_table_from_entity(inventory_balance::Entity),
        ),
        (
            "inventory_transactions",
            schema.create_table_from_entity(inventory_transaction::Entity),
        ),
        (
            "sales_order_headers",
            schema.create_table_from_entity(sales_order_header::Entity),
        ),
        (
            "sales_order_lines",
            schema.create_table_from_entity(sales_order_line::Entity),
        ),
        (
            "purchase_order_headers",
            schema.create_table_from_entity(purchase_order_headers::Entity),
        ),
        (
            "purchase_order_lines",
            schema.create_table_from_entity(purchase_order_lines::Entity),
        ),
        (
            "po_receipt_headers",
            schema.create_table_from_entity(po_receipt_headers::Entity),
        ),
        (
            "po_receipt_lines",
            schema.create_table_from_entity(po_receipt_lines::Entity),
        ),
        (
            "order_fulfillments",
            schema.create_table_from_entity(order_fulfillments::Entity),
        ),
        (
            "shipments",
            schema.create_table_from_entity(shipment::Entity),
        ),
        ("orders", schema.create_table_from_entity(order::Entity)),
        (
            "order_items",
            schema.create_table_from_entity(order_item::Entity),
        ),
        ("products", schema.create_table_from_entity(product::Entity)),
        (
            "product_variants",
            schema.create_table_from_entity(commerce_product_variant::Entity),
        ),
        (
            "customers",
            schema.create_table_from_entity(commerce_customer::Entity),
        ),
        (
            "customer_addresses",
            schema.create_table_from_entity(commerce_customer_address::Entity),
        ),
        ("users", schema.create_table_from_entity(user::Entity)),
        (
            "user_roles",
            schema.create_table_from_entity(user_role::Entity),
        ),
        (
            "refresh_tokens",
            schema.create_table_from_entity(refresh_token::Entity),
        ),
        ("api_keys", schema.create_table_from_entity(api_key::Entity)),
    ];

    if backend == DbBackend::Sqlite {
        if let Some(outbox) = sqlite_outbox_table() {
            tables.push(("outbox_events", outbox));
        }
    }

    tables
}

fn sqlite_outbox_table() -> Option<TableCreateStatement> {
    use sea_orm::sea_query::{Alias, ColumnDef, Expr, Table};

    let table = Table::create()
        .table(Alias::new("outbox_events"))
        .if_not_exists()
        .col(
            ColumnDef::new(Alias::new("id"))
                .uuid()
                .not_null()
                .primary_key(),
        )
        .col(
            ColumnDef::new(Alias::new("aggregate_type"))
                .string()
                .not_null(),
        )
        .col(ColumnDef::new(Alias::new("aggregate_id")).uuid().null())
        .col(ColumnDef::new(Alias::new("event_type")).string().not_null())
        .col(ColumnDef::new(Alias::new("payload")).json().not_null())
        .col(ColumnDef::new(Alias::new("headers")).json().null())
        .col(
            ColumnDef::new(Alias::new("status"))
                .string()
                .not_null()
                .default("pending"),
        )
        .col(
            ColumnDef::new(Alias::new("attempts"))
                .integer()
                .not_null()
                .default(0),
        )
        .col(
            ColumnDef::new(Alias::new("available_at"))
                .timestamp()
                .not_null()
                .default(Expr::current_timestamp()),
        )
        .col(
            ColumnDef::new(Alias::new("processed_at"))
                .timestamp()
                .null(),
        )
        .col(ColumnDef::new(Alias::new("error_message")).string().null())
        .col(ColumnDef::new(Alias::new("partition_key")).string().null())
        .col(
            ColumnDef::new(Alias::new("created_at"))
                .timestamp()
                .not_null()
                .default(Expr::current_timestamp()),
        )
        .col(ColumnDef::new(Alias::new("updated_at")).timestamp().null())
        .to_owned();

    Some(table)
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
    let backend = pool.get_database_backend();

    // Execute migrations using our embedded migrator
    let migrate_result = crate::migrator::Migrator::up(pool, None).await;

    let elapsed = start.elapsed();
    match &migrate_result {
        Ok(_) => {
            if backend != DbBackend::Sqlite {
                info!(
                    "Embedded database migrations completed successfully in {:?}",
                    elapsed
                );
            }
        }
        Err(e) => error!(
            "Embedded database migrations failed after {:?}: {}",
            elapsed, e
        ),
    }

    let ensure_start = std::time::Instant::now();
    let ensure_result = ensure_core_tables(pool).await;
    let ensure_elapsed = ensure_start.elapsed();

    match &ensure_result {
        Ok(_) => info!("Verified core inventory tables in {:?}", ensure_elapsed),
        Err(e) => error!(
            "Ensuring core inventory tables failed after {:?}: {}",
            ensure_elapsed, e
        ),
    }

    migrate_result.map_err(AppError::DatabaseError)?;
    ensure_result?;

    Ok(())
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
