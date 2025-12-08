/*!
 * # Health Check Module
 *
 * This module provides endpoints for monitoring the health and readiness of the Stateset API.
 * It includes:
 *
 * - Basic health check (`/health`) - Simple up/down status
 * - Readiness check (`/health/ready`) - Checks if the system is ready to accept traffic
 * - Liveness check (`/health/live`) - Checks if the system is alive
 * - Detailed health check (`/health/details`) - Provides detailed system information
 */

use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Json},
    routing::get,
    Router,
};
use chrono::{DateTime, Utc};
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::RwLock;
use tracing::{error, info, warn};

/// Basic health status
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum HealthStatus {
    Up,
    Down,
    Degraded,
}

/// Health check detail
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct HealthDetail {
    pub status: HealthStatus,
    pub message: Option<String>,
    pub timestamp: DateTime<Utc>,
}

/// Overall health information
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct HealthInfo {
    pub status: HealthStatus,
    pub version: String,
    pub timestamp: DateTime<Utc>,
    pub uptime_seconds: u64,
    pub details: HashMap<String, HealthDetail>,
}

/// Health check state
#[derive(Clone)]
pub struct HealthState {
    pub db_pool: Arc<DatabaseConnection>,
    pub health_cache: Arc<RwLock<HealthInfo>>,
    pub start_time: SystemTime,
}

impl HealthState {
    pub fn new(db_pool: Arc<DatabaseConnection>) -> Self {
        Self {
            db_pool,
            health_cache: Arc::new(RwLock::new(HealthInfo {
                status: HealthStatus::Up,
                version: env!("CARGO_PKG_VERSION").to_string(),
                timestamp: Utc::now(),
                uptime_seconds: 0,
                details: HashMap::new(),
            })),
            start_time: SystemTime::now(),
        }
    }

    /// Calculate system uptime
    pub fn uptime(&self) -> u64 {
        SystemTime::now()
            .duration_since(self.start_time)
            .unwrap_or(Duration::from_secs(0))
            .as_secs()
    }

    /// Update health status
    pub async fn update_health(&self) {
        let mut health = self.health_cache.write().await;
        health.timestamp = Utc::now();
        health.uptime_seconds = self.uptime();

        // Check database connection
        health.details.insert(
            "database".to_string(),
            HealthDetail {
                status: match self.db_pool.ping().await {
                    Ok(_) => HealthStatus::Up,
                    Err(e) => {
                        error!("Database health check failed: {}", e);
                        HealthStatus::Down
                    }
                },
                message: None,
                timestamp: Utc::now(),
            },
        );

        // Check Redis connection (if you have a Redis client as part of your state)
        // This is a placeholder - you'll need to adjust based on your actual Redis client
        health.details.insert(
            "redis".to_string(),
            HealthDetail {
                status: HealthStatus::Up, // Replace with actual check
                message: None,
                timestamp: Utc::now(),
            },
        );

        // Compute overall status
        let any_down = health
            .details
            .values()
            .any(|detail| detail.status == HealthStatus::Down);
        let any_degraded = health
            .details
            .values()
            .any(|detail| detail.status == HealthStatus::Degraded);

        health.status = if any_down {
            HealthStatus::Down
        } else if any_degraded {
            HealthStatus::Degraded
        } else {
            HealthStatus::Up
        };
    }
}

/// Returns build and version information
pub async fn version_info() -> impl IntoResponse {
    Json(json!({
        "version": env!("CARGO_PKG_VERSION"),
        "commit": option_env!("GIT_HASH").unwrap_or("unknown"),
        "built": option_env!("BUILD_TIME").unwrap_or("unknown"),
    }))
}

/// Basic health check endpoint
pub async fn health_check(State(state): State<Arc<HealthState>>) -> impl IntoResponse {
    info!("Health check endpoint called");

    let health = state.health_cache.read().await;

    let status_code = match health.status {
        HealthStatus::Up => StatusCode::OK,
        HealthStatus::Degraded => StatusCode::OK,
        HealthStatus::Down => StatusCode::SERVICE_UNAVAILABLE,
    };

    (
        status_code,
        Json(json!({
            "status": health.status,
            "version": health.version,
            "timestamp": health.timestamp,
        })),
    )
}

/// Readiness check endpoint
pub async fn readiness_check(State(state): State<Arc<HealthState>>) -> impl IntoResponse {
    info!("Readiness check endpoint called");

    // Update health state before responding
    state.update_health().await;
    let health = state.health_cache.read().await;

    let status_code = match health.status {
        HealthStatus::Up => StatusCode::OK,
        HealthStatus::Degraded => StatusCode::OK,
        HealthStatus::Down => StatusCode::SERVICE_UNAVAILABLE,
    };

    (
        status_code,
        Json(json!({
            "ready": health.status == HealthStatus::Up,
            "timestamp": health.timestamp,
        })),
    )
}

/// Liveness check endpoint
pub async fn liveness_check(State(state): State<Arc<HealthState>>) -> impl IntoResponse {
    info!("Liveness check endpoint called");

    let health = state.health_cache.read().await;

    (
        StatusCode::OK,
        Json(json!({
            "alive": true,
            "uptime_seconds": health.uptime_seconds,
            "timestamp": health.timestamp,
        })),
    )
}

/// Detailed health check endpoint
pub async fn detailed_health(State(state): State<Arc<HealthState>>) -> impl IntoResponse {
    info!("Detailed health check endpoint called");

    // Update health state before responding
    state.update_health().await;
    let health = state.health_cache.read().await;

    let status_code = match health.status {
        HealthStatus::Up => StatusCode::OK,
        HealthStatus::Degraded => StatusCode::OK,
        HealthStatus::Down => StatusCode::SERVICE_UNAVAILABLE,
    };

    (status_code, Json(health.clone()))
}

// Readiness check with database verification available via health_check_detailed endpoint

/// Run periodic health checks
pub async fn run_health_checker(state: Arc<HealthState>) {
    info!("Starting periodic health checker");

    let mut interval = tokio::time::interval(Duration::from_secs(30));

    loop {
        interval.tick().await;
        state.update_health().await;

        let health = state.health_cache.read().await;
        if health.status != HealthStatus::Up {
            warn!("System health is not optimal: {:?}", health.status);

            for (name, detail) in &health.details {
                if detail.status != HealthStatus::Up {
                    warn!("Component {name} is not healthy: {:?}", detail.status);
                }
            }
        }
    }
}

/// Creates router with health check endpoints (with HealthState)
pub fn health_routes_with_state(db_pool: Arc<DatabaseConnection>) -> Router {
    let health_state = Arc::new(HealthState::new(db_pool));

    // Start the background health checker
    tokio::spawn(run_health_checker(health_state.clone()));

    Router::new()
        .route("/", get(health_check))
        .route("/ready", get(readiness_check))
        .route("/live", get(liveness_check))
        .route("/details", get(detailed_health))
        .route("/version", get(version_info))
        .with_state(health_state)
}

/// Creates router with health check endpoints (simplified version for use with AppState)

pub fn health_routes() -> Router<()> {
    Router::new()
        .route("/", get(simple_health_check))
        .route("/readiness", get(simple_health_check))
        .route("/version", get(version_info))
}

/// Simple health check response that doesn't require state
pub async fn simple_health_check() -> impl IntoResponse {
    info!("Health check endpoint called");

    (
        StatusCode::OK,
        Json(json!({
            "status": "up",
            "version": env!("CARGO_PKG_VERSION"),
            "timestamp": chrono::Utc::now().to_rfc3339(),
        })),
    )
}
