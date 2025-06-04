use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Json},
    routing::get,
    Router,
};
use serde_json::json;
use std::sync::Arc;
use tracing::info;

use crate::{db, AppState};

/// Returns build and version information
pub async fn version_info() -> impl IntoResponse {
    Json(json!({
        "version": env!("CARGO_PKG_VERSION"),
        "commit": env!("GIT_HASH"),
        "built": env!("BUILD_TIME"),
    }))
}

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

/// Readiness check that verifies database connectivity
pub async fn readiness_check(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    match db::check_connection(&state.db).await {
        Ok(_) => (
            StatusCode::OK,
            Json(json!({
                "status": "ready",
                "database": "up",
                "timestamp": chrono::Utc::now().to_rfc3339(),
            })),
        ),
        Err(err) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({
                "status": "degraded",
                "database": "down",
                "error": err.to_string(),
                "timestamp": chrono::Utc::now().to_rfc3339(),
            })),
        ),
    }
}

/// Creates a router with the health check endpoint
pub fn health_routes() -> Router {
    Router::new()
        .route("/", get(health_check))
        .route("/readiness", get(readiness_check))
        .route("/version", get(version_info))
}