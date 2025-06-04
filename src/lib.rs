//! StateSet API Library
//!
//! This crate provides the core functionality for the StateSet API

pub mod auth;
pub mod config;
pub mod db;
pub mod errors;
pub mod events;
pub mod handlers;
pub mod models;
pub mod commands;
pub mod proto;
pub mod grpc;
pub mod queries;
pub mod repositories;
pub mod schema;
pub mod services;
pub mod entities;
pub mod tracing;

/// Public re-exports for convenient access to commonly used items
pub mod prelude {
    pub use crate::models::*;
    pub use crate::services::*;
    pub use crate::commands::*;
    pub use crate::queries::*;
    pub use crate::errors::*;
    pub use crate::db::*;
    pub use crate::events::*;
}
