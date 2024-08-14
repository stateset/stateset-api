//! Stateset API Library
//! 
//! This library provides the core functionality for the Stateset API,
//! including database models, services, and utilities for caching and rate limiting.

// Re-export diesel macros
#[macro_use]
extern crate diesel;

// Module declarations
pub mod schema;
pub mod models;
pub mod services;
pub mod commands;
pub mod queries;
pub mod errors;
pub mod cache;
pub mod rate_limiter;

// Public re-exports
pub use schema::*;
pub use models::*;
pub use services::*;
pub use commands::*;
pub use queries::*;
pub use errors::*;
pub use cache::Cache;
pub use rate_limiter::RateLimiter;

// If you have any common types or functions that should be available at the crate root, you can add them here
pub use diesel::prelude::*;
pub use redis::Client as RedisClient;

/// Convenience type alias for a database connection pool
pub type DbPool = diesel::r2d2::Pool<diesel::r2d2::ConnectionManager<diesel::PgConnection>>;

/// Initializes the database connection pool
pub fn init_pool(database_url: &str) -> DbPool {
    let manager = diesel::r2d2::ConnectionManager::<diesel::PgConnection>::new(database_url);
    diesel::r2d2::Pool::builder()
        .build(manager)
        .expect("Failed to create pool")
}