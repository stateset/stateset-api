//! Stateset API Library
//! 
//! This library provides the core functionality for the Stateset API,
//! including database models, services, and utilities for caching and rate limiting.

pub mod schema; // This might need to be updated or removed depending on your SeaORM setup
pub mod models;
pub mod services;
pub mod commands;
pub mod queries;
pub mod errors;
pub mod cache;
pub mod rate_limiter;

// Public re-exports
pub use models::*;
pub use services::*;
pub use commands::*;
pub use queries::*;
pub use errors::*;
pub use cache::Cache;
pub use rate_limiter::RateLimiter;

pub use sea_orm::{DatabaseConnection, EntityTrait, QueryFilter};
pub use redis::Client as RedisClient;

/// Convenience type alias for a database connection
pub type DbPool = DatabaseConnection;

/// Initializes the database connection
pub async fn init_pool(database_url: &str) -> DbPool {
    sea_orm::Database::connect(database_url)
        .await
        .expect("Failed to create database connection")
}
