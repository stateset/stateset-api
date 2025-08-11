use axum::{
    routing::get,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Json},
    Router,
};
use std::sync::Arc;
use serde_json::json;
use std::time::{Duration, Instant};
use crate::{
    errors::ApiError,
    handlers::AppState,
};
use tracing::info;

/// Basic health check response
async fn health_check() -> impl IntoResponse {
    Json(json!({
        "status": "up",
        "version": env!("CARGO_PKG_VERSION"),
        "timestamp": chrono::Utc::now().to_rfc3339()
    }))
}

/// Enhanced health check that tests database connection
async fn detailed_health_check(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, ApiError> {
    let start = Instant::now();
    
    // Check database connection
    let db_result = crate::db::check_connection(&state.db_pool).await;
    let db_status = db_result.is_ok();
    
    let duration = start.elapsed();
    let all_services_up = db_status;
    
    let status_code = if all_services_up {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    
    let response = Json(json!({
        "status": if all_services_up { "up" } else { "degraded" },
        "version": env!("CARGO_PKG_VERSION"),
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "details": {
            "database": {
                "status": if db_status { "up" } else { "down" },
                "message": db_result.map_or_else(|e| e.to_string(), |_| "Connection successful".to_string())
            }
        },
        "response_time_ms": duration.as_millis()
    }));
    
    Ok((status_code, response))
}

/// Creates the router for health check endpoints
pub fn health_routes() -> Router {
    Router::new()
        .route("/", get(health_check))
        .route("/detailed", get(detailed_health_check))
}
