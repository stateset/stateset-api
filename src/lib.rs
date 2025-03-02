//! StateSet API Library
//!
//! This crate provides the core functionality for the StateSet API, including:
//! - Database models and schema definitions
//! - Service implementations for business logic
//! - Command and query patterns for data operations
//! - Error handling utilities
//! - Caching and rate limiting mechanisms
//!
//! The library is designed to be modular and extensible, providing a foundation
//! for building robust API services with proper separation of concerns.

#![deny(missing_docs)] // Enforce documentation for all public items
#![warn(clippy::all)] // Enable all clippy lints for code quality

pub mod schema; // Note: Consider replacing with SeaORM entities if using codegen
pub mod models;
pub mod services;
pub mod commands;
pub mod queries;
pub mod errors;
pub mod cache;
pub mod rate_limiter;
pub mod db;
pub mod events;

/// Core database connection type from SeaORM
pub use sea_orm::{
    DatabaseConnection,
    EntityTrait,
    QueryFilter,
    DbErr,
};

/// Redis client type
pub use redis::Client as RedisClient;

/// Public re-exports for convenient access to commonly used items
pub mod prelude {
    pub use super::models::*;
    pub use super::services::*;
    pub use super::commands::*;
    pub use super::queries::*;
    pub use super::errors::{ServiceError, *};
    pub use super::cache::Cache;
    pub use super::rate_limiter::RateLimiter;
    pub use super::db::*;
    pub use super::events::*;
}

/// Convenience type alias for database connection pool
pub type DbPool = DatabaseConnection;

/// Initializes a database connection pool
///
/// # Arguments
/// * `database_url` - The connection string for the database (e.g., "postgres://user:pass@localhost/db")
///
/// # Examples
/// ```rust,no_run
/// use stateset_api::init_pool;
/// 
/// # #[tokio::main]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let pool = init_pool("postgres://user:password@localhost/stateset").await?;
/// # Ok(())
/// # }
/// ```
///
/// # Errors
/// Returns a `DbErr` if the connection cannot be established
pub async fn init_pool(database_url: &str) -> Result<DbPool, DbErr> {
    sea_orm::Database::connect(database_url)
        .await
        .map_err(|e| {
            log::error!("Failed to initialize database pool: {}", e);
            e
        })
}

/// Configuration struct for library initialization
#[derive(Debug, Clone)]
pub struct Config {
    /// Database connection URL
    pub database_url: String,
    /// Redis connection URL
    pub redis_url: String,
    /// Maximum number of database connections
    pub max_connections: u32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            database_url: String::from("postgres://localhost/stateset"),
            redis_url: String::from("redis://localhost"),
            max_connections: 10,
        }
    }
}

/// Initializes the library with custom configuration
///
/// # Arguments
/// * `config` - Custom configuration for the library
///
/// # Returns
/// A tuple containing the database pool and redis client
pub async fn initialize(config: Config) -> Result<(DbPool, RedisClient), anyhow::Error> {
    let pool = sea_orm::Database::connect(
        sea_orm::ConnectOptions::new(&config.database_url)
            .max_connections(config.max_connections)
            .to_owned()
    ).await?;
    
    let redis_client = redis::Client::open(config.redis_url)?;
    
    Ok((pool, redis_client))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore] // Ignored as it requires a running database
    async fn test_init_pool() {
        let url = "postgres://user:password@localhost/test";
        let result = init_pool(url).await;
        assert!(result.is_ok());
    }
}