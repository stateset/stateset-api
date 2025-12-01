use axum::{
    routing::get,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Json},
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use serde_json::json;
use std::time::Instant;
use crate::{
    errors::ApiError,
    handlers::AppState,
};

/// Component health status
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ComponentStatus {
    Up,
    Down,
    Degraded,
}

/// Individual component health details
#[derive(Debug, Clone, Serialize)]
pub struct ComponentHealth {
    pub status: ComponentStatus,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latency_ms: Option<u64>,
}

/// Full health check response
#[derive(Debug, Clone, Serialize)]
pub struct HealthResponse {
    pub status: ComponentStatus,
    pub version: String,
    pub timestamp: String,
    pub uptime_secs: u64,
    pub details: HealthDetails,
    pub response_time_ms: u128,
}

#[derive(Debug, Clone, Serialize)]
pub struct HealthDetails {
    pub database: ComponentHealth,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub redis: Option<ComponentHealth>,
}

/// Tracks application start time for uptime calculation
static START_TIME: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();

/// Initialize the start time (call this on application startup)
pub fn init_start_time() {
    let _ = START_TIME.get_or_init(Instant::now);
}

fn get_uptime_secs() -> u64 {
    START_TIME.get().map(|t| t.elapsed().as_secs()).unwrap_or(0)
}

/// Basic liveness probe - just checks if the service is running
/// Kubernetes uses this to know if the container is alive
async fn liveness_check() -> impl IntoResponse {
    Json(json!({
        "status": "up",
        "version": env!("CARGO_PKG_VERSION"),
        "timestamp": chrono::Utc::now().to_rfc3339()
    }))
}

/// Readiness probe - checks if the service is ready to handle traffic
/// Kubernetes uses this to know if traffic should be routed to this pod
async fn readiness_check(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let start = Instant::now();

    // Check database connection with timeout
    let db_check_start = Instant::now();
    let db_result = crate::db::check_connection(&state.db).await;
    let db_latency = db_check_start.elapsed().as_millis() as u64;

    let is_ready = db_result.is_ok();

    if is_ready {
        Ok((StatusCode::OK, Json(json!({
            "status": "ready",
            "checks": {
                "database": {
                    "status": "up",
                    "latency_ms": db_latency
                }
            },
            "response_time_ms": start.elapsed().as_millis()
        }))))
    } else {
        Err((StatusCode::SERVICE_UNAVAILABLE, Json(json!({
            "status": "not_ready",
            "checks": {
                "database": {
                    "status": "down",
                    "error": db_result.err().map(|e| e.to_string())
                }
            },
            "response_time_ms": start.elapsed().as_millis()
        }))))
    }
}

/// Enhanced health check that tests all dependencies
/// Returns comprehensive status of all system components
async fn detailed_health_check(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, ApiError> {
    let start = Instant::now();

    // Check database connection
    let db_check_start = Instant::now();
    let db_result = crate::db::check_connection(&state.db).await;
    let db_latency = db_check_start.elapsed().as_millis() as u64;
    let db_status = db_result.is_ok();

    let db_health = ComponentHealth {
        status: if db_status { ComponentStatus::Up } else { ComponentStatus::Down },
        message: db_result.map_or_else(
            |e| format!("Connection failed: {}", e),
            |_| "Connection successful".to_string()
        ),
        latency_ms: Some(db_latency),
    };

    // Check Redis
    let redis_check_start = Instant::now();
    let redis_result = check_redis_connection(&state.redis).await;
    let redis_latency = redis_check_start.elapsed().as_millis() as u64;

    let redis_health = Some(ComponentHealth {
        status: if redis_result.is_ok() { ComponentStatus::Up } else { ComponentStatus::Down },
        message: redis_result.map_or_else(
            |e| format!("Connection failed: {}", e),
            |_| "Connection successful".to_string()
        ),
        latency_ms: Some(redis_latency),
    });

    let duration = start.elapsed();

    // Determine overall status
    let all_critical_up = db_status;
    let all_optional_up = redis_health.as_ref().map_or(true, |r| matches!(r.status, ComponentStatus::Up));

    let overall_status = if all_critical_up && all_optional_up {
        ComponentStatus::Up
    } else if all_critical_up {
        ComponentStatus::Degraded
    } else {
        ComponentStatus::Down
    };

    let status_code = match overall_status {
        ComponentStatus::Up => StatusCode::OK,
        ComponentStatus::Degraded => StatusCode::OK, // Still serve traffic but alert
        ComponentStatus::Down => StatusCode::SERVICE_UNAVAILABLE,
    };

    let response = HealthResponse {
        status: overall_status,
        version: env!("CARGO_PKG_VERSION").to_string(),
        timestamp: chrono::Utc::now().to_rfc3339(),
        uptime_secs: get_uptime_secs(),
        details: HealthDetails {
            database: db_health,
            redis: redis_health,
        },
        response_time_ms: duration.as_millis(),
    };

    Ok((status_code, Json(response)))
}

/// Check Redis connection
async fn check_redis_connection(client: &redis::Client) -> Result<(), String> {
    

    let mut conn = client
        .get_async_connection()
        .await
        .map_err(|e| format!("Failed to connect: {}", e))?;

    let _: String = redis::cmd("PING")
        .query_async(&mut conn)
        .await
        .map_err(|e| format!("Ping failed: {}", e))?;

    Ok(())
}

/// Creates the router for health check endpoints
///
/// Endpoints:
/// - GET /health         - Basic liveness probe (always returns 200 if server is running)
/// - GET /health/ready   - Readiness probe (checks database connectivity)
/// - GET /health/detailed - Full health check with all component statuses
pub fn health_routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(liveness_check))
        .route("/ready", get(readiness_check))
        .route("/detailed", get(detailed_health_check))
}
