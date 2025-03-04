use axum::{
    routing::get,
    response::{IntoResponse, Json},
    Router,
};
use serde_json::json;
use tracing::info;

/// Basic health check response
pub async fn health_check() -> impl IntoResponse {
    info!("Health check endpoint called");
    
    // Print detailed information
    tracing::debug!("Serving health check request");
    
    Json(json!({
        "status": "up",
        "version": env!("CARGO_PKG_VERSION"),
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "server": "Stateset API"
    }))
}

/// Creates a router with the health check endpoint
pub fn health_routes() -> Router {
    Router::new()
        .route("/", get(health_check))
}