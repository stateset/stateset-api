//! StateSet API Library
//!
//! This crate provides the core functionality for the StateSet API

// Core modules
pub mod api;
pub mod auth;
pub mod cache;
pub mod circuit_breaker;
// pub mod commands;
pub mod config;
pub mod db;
pub mod entities;
pub mod errors;
pub mod events;
pub mod handlers;
pub mod health;
pub mod message_queue;
pub mod metrics;
// pub mod models;
pub mod openapi;
pub mod proto;
pub mod rate_limiter;
pub mod services;
pub mod tracing;
pub mod versioning;

use axum::{
    extract::State,
    middleware,
    response::Json,
    routing::get,
    Router,
};
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;
use utoipa::ToSchema;

// Tracing imports - use external tracing crate directly to avoid conflicts

// Import handler traits
use handlers::inventory::InventoryHandlerState;

// App state definition
#[derive(Clone)]
pub struct AppState {
    pub db: Arc<DatabaseConnection>,
    pub config: config::AppConfig,
    pub event_sender: events::EventSender,
    pub inventory_service: services::inventory::InventoryService,
}

// Common query parameters for list endpoints
#[derive(Debug, Deserialize, ToSchema)]
pub struct ListQuery {
    #[serde(default = "default_page")]
    pub page: u64,
    #[serde(default = "default_limit")]
    pub limit: u64,
    pub search: Option<String>,
    pub sort_by: Option<String>,
    pub sort_order: Option<String>,
}

fn default_page() -> u64 { 1 }
fn default_limit() -> u64 { 20 }

// Common response wrappers
#[derive(Serialize, ToSchema)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub message: Option<String>,
    pub errors: Option<Vec<String>>,
}

#[derive(Serialize, ToSchema)]
pub struct PaginatedResponse<T> {
    pub items: Vec<T>,
    pub total: u64,
    pub page: u64,
    pub limit: u64,
    pub total_pages: u64,
}

impl<T> ApiResponse<T> {
    pub fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            message: None,
            errors: None,
        }
    }

    pub fn error(message: String) -> Self {
        Self {
            success: false,
            data: None,
            message: Some(message),
            errors: None,
        }
    }

    pub fn validation_errors(errors: Vec<String>) -> Self {
        Self {
            success: false,
            data: None,
            message: Some("Validation failed".to_string()),
            errors: Some(errors),
        }
    }
}

// Enhanced API routes function
pub fn api_v1_routes() -> Router<AppState> {
    Router::new()
        // Status and health endpoints
        .route("/status", get(api_status))
        .route("/health", get(health_check))
        
        // Inventory API (working endpoints)
        .route("/inventory", get(handlers::inventory::list_inventory::<AppState>))
        .route("/inventory", axum::routing::post(handlers::inventory::create_inventory::<AppState>))
        .route("/inventory/:id", get(handlers::inventory::get_inventory::<AppState>))
        .route("/inventory/:id", axum::routing::put(handlers::inventory::update_inventory::<AppState>))
        .route("/inventory/:id", axum::routing::delete(handlers::inventory::delete_inventory::<AppState>))
        .route("/inventory/:id/reserve", axum::routing::post(handlers::inventory::reserve_inventory::<AppState>))
        .route("/inventory/:id/release", axum::routing::post(handlers::inventory::release_inventory::<AppState>))
        .route("/inventory/low-stock", get(handlers::inventory::get_low_stock_items::<AppState>))
        
        // Returns API
        .route("/returns", get(handlers::returns::list_returns::<AppState>))
        .route("/returns", axum::routing::post(handlers::returns::create_return::<AppState>))
        .route("/returns/:id", get(handlers::returns::get_return::<AppState>))
        .route("/returns/:id", axum::routing::put(handlers::returns::update_return::<AppState>))
        .route("/returns/:id", axum::routing::delete(handlers::returns::delete_return::<AppState>))
        .route("/returns/:id/status", axum::routing::put(handlers::returns::update_return_status::<AppState>))
        .route("/returns/:id/process", axum::routing::post(handlers::returns::process_return::<AppState>))
        
        // Shipments API
        .route("/shipments", get(handlers::shipments::list_shipments::<AppState>))
        .route("/shipments", axum::routing::post(handlers::shipments::create_shipment::<AppState>))
        .route("/shipments/:id", get(handlers::shipments::get_shipment::<AppState>))
        .route("/shipments/:id", axum::routing::put(handlers::shipments::update_shipment::<AppState>))
        .route("/shipments/:id", axum::routing::delete(handlers::shipments::delete_shipment::<AppState>))
        .route("/shipments/:id/track", get(handlers::shipments::track_shipment::<AppState>))
        .route("/shipments/:id/status", axum::routing::put(handlers::shipments::update_shipment_status::<AppState>))
        
        // Warranties API
        .route("/warranties", get(handlers::warranties::list_warranties::<AppState>))
        .route("/warranties", axum::routing::post(handlers::warranties::create_warranty::<AppState>))
        .route("/warranties/:id", get(handlers::warranties::get_warranty::<AppState>))
        .route("/warranties/:id", axum::routing::put(handlers::warranties::update_warranty::<AppState>))
        .route("/warranties/:id", axum::routing::delete(handlers::warranties::delete_warranty::<AppState>))
        .route("/warranties/:id/claim", axum::routing::post(handlers::warranties::create_warranty_claim::<AppState>))
        
        // Work Orders API
        .route("/work-orders", get(handlers::work_orders::list_work_orders::<AppState>))
        .route("/work-orders", axum::routing::post(handlers::work_orders::create_work_order::<AppState>))
        .route("/work-orders/:id", get(handlers::work_orders::get_work_order::<AppState>))
        .route("/work-orders/:id", axum::routing::put(handlers::work_orders::update_work_order::<AppState>))
        .route("/work-orders/:id", axum::routing::delete(handlers::work_orders::delete_work_order::<AppState>))
        .route("/work-orders/:id/assign", axum::routing::post(handlers::work_orders::assign_work_order::<AppState>))
        .route("/work-orders/:id/complete", axum::routing::post(handlers::work_orders::complete_work_order::<AppState>))
        .route("/work-orders/:id/status", axum::routing::put(handlers::work_orders::update_work_order_status::<AppState>))
        
        // Add middleware for request logging
        .layer(middleware::from_fn(request_logging_middleware))
}

async fn api_status() -> Result<Json<ApiResponse<Value>>, errors::ServiceError> {
    let status_data = json!({
        "status": "ok",
        "version": "1.0.0",
        "service": "stateset-api",
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "environment": std::env::var("ENVIRONMENT").unwrap_or_else(|_| "development".to_string()),
    });
    
    Ok(Json(ApiResponse::success(status_data)))
}

async fn health_check(State(state): State<AppState>) -> Result<Json<ApiResponse<Value>>, errors::ServiceError> {
    // Check database connectivity
    let db_status = match state.db.ping().await {
        Ok(_) => "healthy",
        Err(_) => "unhealthy",
    };
    
    let health_data = json!({
        "status": if db_status == "healthy" { "healthy" } else { "unhealthy" },
        "checks": {
            "database": db_status,
            "cache": "healthy", // TODO: Add actual cache health check
            "message_queue": "healthy", // TODO: Add actual MQ health check
        },
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "uptime": "unknown", // TODO: Calculate actual uptime
    });
    
    Ok(Json(ApiResponse::success(health_data)))
}

// Request logging middleware
async fn request_logging_middleware(
    request: axum::http::Request<axum::body::Body>,
    next: axum::middleware::Next,
) -> axum::response::Response {
    let method = request.method().clone();
    let uri = request.uri().clone();
    let start = std::time::Instant::now();
    
    // Log incoming request
    println!("Incoming request: {} {}", method, uri);
    
    let response = next.run(request).await;
    
    let duration = start.elapsed();
    let status = response.status();
    
    // Log completed request
    println!("Request completed: {} {} - {} in {:?}", method, uri, status, duration);
    
    response
}

pub mod prelude {
    pub use crate::api::*;
    // pub use crate::cache::*;
    // pub use crate::commands::*;
    pub use crate::db::*;
    pub use crate::errors::*;
    pub use crate::events::*;
    pub use crate::health::*;
    pub use crate::metrics::*;
    // pub use crate::models::*;
    pub use crate::openapi::*;
    pub use crate::proto::*;
    // pub use crate::queries::*;
    pub use crate::rate_limiter::*;
    pub use crate::services::*;
    pub use crate::tracing::*;
    pub use crate::versioning::*;
}

// Implement InventoryHandlerState trait for AppState
impl InventoryHandlerState for AppState {
    fn inventory_service(&self) -> &services::inventory::InventoryService {
        &self.inventory_service
    }
}

// Note: AppState automatically implements ReturnsAppState, ShipmentsAppState, 
// WarrantiesAppState, and WorkOrdersAppState through blanket implementations
// in the respective handler modules
