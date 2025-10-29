pub mod query_builder;

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
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::entities::{
    inventory_balance, inventory_location, inventory_transaction, item_master, order_fulfillments,
    po_receipt_headers, po_receipt_lines, purchase_order_headers, purchase_order_lines,
    sales_order_header, sales_order_line,
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

async fn ensure_core_tables(pool: &DbPool) -> Result<(), AppError> {
    match pool.get_database_backend() {
        DbBackend::Sqlite => {
            debug!("Ensuring core tables using SQLite schema helper");
            ensure_core_tables_sqlite(pool).await
        }
        other => {
            debug!(
                backend = ?other,
                "Ensuring core tables using generic schema helper"
            );
            ensure_core_tables_generic(pool).await
        }
    }
}

async fn ensure_core_tables_generic(pool: &DbPool) -> Result<(), AppError> {
    let backend = pool.get_database_backend();
    let schema = Schema::new(backend);

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
    ];

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

async fn ensure_core_tables_sqlite(pool: &DbPool) -> Result<(), AppError> {
    let statements: &[&str] = &[
        r#"
        CREATE TABLE IF NOT EXISTS item_master (
            inventory_item_id INTEGER PRIMARY KEY AUTOINCREMENT,
            organization_id INTEGER NOT NULL,
            item_number TEXT NOT NULL,
            description TEXT,
            primary_uom_code TEXT,
            item_type TEXT,
            status_code TEXT,
            lead_time_weeks INTEGER,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        "#,
        r#"
        CREATE TABLE IF NOT EXISTS inventory_locations (
            location_id INTEGER PRIMARY KEY AUTOINCREMENT,
            location_name TEXT NOT NULL
        );
        "#,
        r#"
        CREATE TABLE IF NOT EXISTS inventory_balances (
            inventory_balance_id INTEGER PRIMARY KEY AUTOINCREMENT,
            inventory_item_id INTEGER NOT NULL,
            location_id INTEGER NOT NULL,
            quantity_on_hand NUMERIC NOT NULL DEFAULT 0,
            quantity_allocated NUMERIC NOT NULL DEFAULT 0,
            quantity_available NUMERIC NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            FOREIGN KEY (inventory_item_id) REFERENCES item_master(inventory_item_id),
            FOREIGN KEY (location_id) REFERENCES inventory_locations(location_id),
            UNIQUE (inventory_item_id, location_id)
        );
        "#,
        r#"
        CREATE TABLE IF NOT EXISTS inventory_transactions (
            id TEXT PRIMARY KEY,
            product_id TEXT NOT NULL,
            location_id TEXT NOT NULL,
            type TEXT NOT NULL,
            quantity INTEGER NOT NULL,
            previous_quantity INTEGER NOT NULL,
            new_quantity INTEGER NOT NULL,
            reference_id TEXT,
            reference_type TEXT,
            reason TEXT,
            notes TEXT,
            created_by TEXT NOT NULL,
            created_at TEXT NOT NULL
        );
        "#,
        r#"
        CREATE TABLE IF NOT EXISTS sales_order_headers (
            header_id INTEGER PRIMARY KEY AUTOINCREMENT,
            order_number TEXT NOT NULL,
            order_type_id INTEGER,
            sold_to_org_id INTEGER,
            ordered_date TEXT,
            status_code TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            location_id INTEGER,
            FOREIGN KEY (location_id) REFERENCES inventory_locations(location_id)
        );
        "#,
        r#"
        CREATE TABLE IF NOT EXISTS sales_order_lines (
            line_id INTEGER PRIMARY KEY AUTOINCREMENT,
            header_id INTEGER,
            line_number INTEGER,
            inventory_item_id INTEGER,
            ordered_quantity NUMERIC,
            unit_selling_price NUMERIC,
            line_status TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            location_id INTEGER,
            FOREIGN KEY (header_id) REFERENCES sales_order_headers(header_id),
            FOREIGN KEY (inventory_item_id) REFERENCES item_master(inventory_item_id),
            FOREIGN KEY (location_id) REFERENCES inventory_locations(location_id)
        );
        "#,
        r#"
        CREATE TABLE IF NOT EXISTS purchase_order_headers (
            po_header_id INTEGER PRIMARY KEY AUTOINCREMENT,
            po_number TEXT NOT NULL,
            type_code TEXT,
            vendor_id INTEGER,
            agent_id INTEGER,
            approved_flag INTEGER,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        "#,
        r#"
        CREATE TABLE IF NOT EXISTS purchase_order_lines (
            po_line_id INTEGER PRIMARY KEY AUTOINCREMENT,
            po_header_id INTEGER,
            line_num INTEGER,
            item_id INTEGER,
            quantity NUMERIC,
            unit_price NUMERIC,
            line_type_id INTEGER,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            FOREIGN KEY (po_header_id) REFERENCES purchase_order_headers(po_header_id),
            FOREIGN KEY (item_id) REFERENCES item_master(inventory_item_id)
        );
        "#,
        r#"
        CREATE TABLE IF NOT EXISTS po_receipt_headers (
            shipment_header_id INTEGER PRIMARY KEY AUTOINCREMENT,
            receipt_num TEXT NOT NULL,
            vendor_id INTEGER,
            shipment_num TEXT,
            receipt_source TEXT,
            created_at TEXT,
            updated_at TEXT
        );
        "#,
        r#"
        CREATE TABLE IF NOT EXISTS po_receipt_lines (
            shipment_line_id INTEGER PRIMARY KEY AUTOINCREMENT,
            shipment_header_id INTEGER,
            item_id INTEGER,
            po_header_id INTEGER,
            po_line_id INTEGER,
            quantity_received NUMERIC,
            created_at TEXT,
            updated_at TEXT,
            FOREIGN KEY (shipment_header_id) REFERENCES po_receipt_headers(shipment_header_id),
            FOREIGN KEY (item_id) REFERENCES item_master(inventory_item_id),
            FOREIGN KEY (po_header_id) REFERENCES purchase_order_headers(po_header_id),
            FOREIGN KEY (po_line_id) REFERENCES purchase_order_lines(po_line_id)
        );
        "#,
        r#"
        CREATE TABLE IF NOT EXISTS order_fulfillments (
            fulfillment_id INTEGER PRIMARY KEY AUTOINCREMENT,
            sales_order_header_id INTEGER,
            sales_order_line_id INTEGER,
            shipped_date TEXT,
            released_status TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            FOREIGN KEY (sales_order_header_id) REFERENCES sales_order_headers(header_id),
            FOREIGN KEY (sales_order_line_id) REFERENCES sales_order_lines(line_id)
        );
        "#,
        r#"
        CREATE TABLE IF NOT EXISTS shipments (
            id TEXT PRIMARY KEY,
            order_id TEXT NOT NULL,
            tracking_number TEXT NOT NULL,
            carrier TEXT NOT NULL,
            status TEXT NOT NULL,
            shipping_address TEXT NOT NULL,
            shipping_method TEXT NOT NULL,
            weight_kg REAL,
            dimensions_cm TEXT,
            notes TEXT,
            shipped_at TEXT,
            estimated_delivery TEXT,
            delivered_at TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            created_by TEXT,
            recipient_name TEXT NOT NULL,
            recipient_email TEXT,
            recipient_phone TEXT,
            tracking_url TEXT,
            shipping_cost NUMERIC,
            insurance_amount NUMERIC,
            is_signature_required INTEGER NOT NULL DEFAULT 0
        );
        "#,
        r#"
        CREATE TABLE IF NOT EXISTS users (
            id BLOB PRIMARY KEY,
            name TEXT NOT NULL,
            email TEXT NOT NULL UNIQUE,
            password_hash TEXT NOT NULL,
            tenant_id TEXT,
            active INTEGER NOT NULL DEFAULT 1,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        "#,
        r#"
        CREATE TABLE IF NOT EXISTS user_roles (
            id BLOB PRIMARY KEY,
            user_id BLOB NOT NULL,
            role_name TEXT NOT NULL,
            created_at TEXT NOT NULL,
            FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
        );
        "#,
        r#"
        CREATE INDEX IF NOT EXISTS idx_user_roles_user_id ON user_roles(user_id);
        "#,
        r#"
        CREATE TABLE IF NOT EXISTS refresh_tokens (
            id BLOB PRIMARY KEY,
            user_id BLOB NOT NULL,
            token_id TEXT NOT NULL UNIQUE,
            created_at TEXT NOT NULL,
            expires_at TEXT NOT NULL,
            revoked INTEGER NOT NULL DEFAULT 0,
            FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
        );
        "#,
        r#"
        CREATE INDEX IF NOT EXISTS idx_refresh_tokens_user_id ON refresh_tokens(user_id);
        "#,
        r#"
        CREATE TABLE IF NOT EXISTS api_keys (
            id BLOB PRIMARY KEY,
            name TEXT NOT NULL,
            key_hash TEXT NOT NULL,
            user_id BLOB NOT NULL,
            tenant_id TEXT,
            created_at TEXT NOT NULL,
            expires_at TEXT,
            last_used_at TEXT,
            revoked INTEGER NOT NULL DEFAULT 0,
            FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
        );
        "#,
        r#"
        CREATE INDEX IF NOT EXISTS idx_api_keys_user_id ON api_keys(user_id);
        "#,
        r#"
        CREATE INDEX IF NOT EXISTS idx_api_keys_revoked ON api_keys(revoked);
        "#,
        r#"
        CREATE TABLE IF NOT EXISTS products (
            id BLOB PRIMARY KEY,
            name TEXT NOT NULL,
            description TEXT,
            sku TEXT NOT NULL UNIQUE,
            price REAL NOT NULL,
            currency TEXT NOT NULL DEFAULT 'USD',
            weight_kg REAL,
            dimensions_cm TEXT,
            barcode TEXT,
            brand TEXT,
            manufacturer TEXT,
            is_active INTEGER NOT NULL DEFAULT 1,
            is_digital INTEGER NOT NULL DEFAULT 0,
            image_url TEXT,
            category_id BLOB,
            reorder_point INTEGER,
            tax_rate REAL,
            cost_price REAL,
            msrp REAL,
            tags TEXT,
            meta_title TEXT,
            meta_description TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT
        );
        "#,
        r#"
        CREATE INDEX IF NOT EXISTS idx_products_is_active ON products(is_active);
        "#,
        r#"
        CREATE TABLE IF NOT EXISTS product_variants (
            id BLOB PRIMARY KEY,
            product_id BLOB NOT NULL,
            sku TEXT NOT NULL UNIQUE,
            name TEXT NOT NULL,
            price REAL NOT NULL,
            compare_at_price REAL,
            cost REAL,
            weight REAL,
            dimensions TEXT,
            options TEXT NOT NULL DEFAULT '{}',
            inventory_tracking INTEGER NOT NULL DEFAULT 1,
            position INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            FOREIGN KEY (product_id) REFERENCES products(id) ON DELETE CASCADE
        );
        "#,
        r#"
        CREATE INDEX IF NOT EXISTS idx_product_variants_product_id ON product_variants(product_id);
        "#,
        r#"
        CREATE TABLE IF NOT EXISTS customers (
            id BLOB PRIMARY KEY,
            email TEXT NOT NULL UNIQUE,
            first_name TEXT NOT NULL,
            last_name TEXT NOT NULL,
            phone TEXT,
            accepts_marketing INTEGER NOT NULL DEFAULT 0,
            customer_group_id BLOB,
            default_shipping_address_id BLOB,
            default_billing_address_id BLOB,
            tags TEXT NOT NULL DEFAULT '[]',
            metadata TEXT,
            email_verified INTEGER NOT NULL DEFAULT 0,
            email_verified_at TEXT,
            status TEXT NOT NULL DEFAULT 'active',
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        "#,
        r#"
        CREATE INDEX IF NOT EXISTS idx_customers_status ON customers(status);
        "#,
        r#"
        CREATE TABLE IF NOT EXISTS customer_addresses (
            id BLOB PRIMARY KEY,
            customer_id BLOB NOT NULL,
            name TEXT,
            company TEXT,
            address_line_1 TEXT NOT NULL,
            address_line_2 TEXT,
            city TEXT NOT NULL,
            province TEXT NOT NULL,
            country_code TEXT NOT NULL,
            postal_code TEXT NOT NULL,
            phone TEXT,
            is_default_shipping INTEGER NOT NULL DEFAULT 0,
            is_default_billing INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            FOREIGN KEY (customer_id) REFERENCES customers(id) ON DELETE CASCADE
        );
        "#,
        r#"
        CREATE INDEX IF NOT EXISTS idx_customer_addresses_customer_id ON customer_addresses(customer_id);
        "#,
        r#"
        CREATE TABLE IF NOT EXISTS orders (
            id BLOB PRIMARY KEY,
            order_number TEXT NOT NULL UNIQUE,
            customer_id BLOB NOT NULL,
            status TEXT NOT NULL,
            order_date TEXT NOT NULL,
            total_amount REAL NOT NULL DEFAULT 0.0,
            currency TEXT NOT NULL DEFAULT 'USD',
            payment_status TEXT NOT NULL DEFAULT 'pending',
            fulfillment_status TEXT NOT NULL DEFAULT 'unfulfilled',
            payment_method TEXT,
            shipping_method TEXT,
            tracking_number TEXT,
            notes TEXT,
            shipping_address TEXT,
            billing_address TEXT,
            is_archived INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL,
            updated_at TEXT,
            version INTEGER NOT NULL DEFAULT 1
        );
        "#,
        r#"
        CREATE INDEX IF NOT EXISTS idx_orders_customer_id ON orders(customer_id);
        "#,
        r#"
        CREATE INDEX IF NOT EXISTS idx_orders_status ON orders(status);
        "#,
        r#"
        CREATE INDEX IF NOT EXISTS idx_orders_created_at ON orders(created_at);
        "#,
        r#"
        CREATE TABLE IF NOT EXISTS order_items (
            id BLOB PRIMARY KEY,
            order_id BLOB NOT NULL,
            product_id BLOB NOT NULL,
            sku TEXT NOT NULL,
            name TEXT NOT NULL,
            quantity INTEGER NOT NULL,
            unit_price REAL NOT NULL,
            total_price REAL NOT NULL,
            discount REAL NOT NULL DEFAULT 0.0,
            tax_rate REAL NOT NULL DEFAULT 0.0,
            tax_amount REAL NOT NULL DEFAULT 0.0,
            status TEXT NOT NULL DEFAULT 'pending',
            notes TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT,
            FOREIGN KEY (order_id) REFERENCES orders(id) ON DELETE CASCADE
        );
        "#,
        r#"
        CREATE INDEX IF NOT EXISTS idx_order_items_order_id ON order_items(order_id);
        "#,
    ];

    for sql in statements {
        if let Err(err) = pool
            .execute(Statement::from_string(
                DbBackend::Sqlite,
                sql.trim().to_owned(),
            ))
            .await
        {
            warn!("Failed to execute schema statement for SQLite: {}", err);
            return Err(AppError::DatabaseError(err));
        }
    }

    Ok(())
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
    let migrate_result = if backend == DbBackend::Sqlite {
        info!("SQLite backend detected; skipping embedded migrator scripts");
        Ok(())
    } else {
        crate::migrator::Migrator::up(pool, None).await
    };

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
