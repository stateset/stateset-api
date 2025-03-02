use axum::{
    extract::State,
    routing::get,
    Json, Router,
};
use serde_json::{json, Value};
use crate::AppState;
use redis::{AsyncCommands, RedisResult};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{instrument, trace, warn};

/// Configuration for health checks
#[derive(Debug, Clone)]
pub struct HealthCheckConfig {
    /// Maximum acceptable duration for database check
    pub db_timeout: Duration,
    /// Maximum acceptable duration for Redis check
    pub redis_timeout: Duration,
}

impl Default for HealthCheckConfig {
    fn default() -> Self {
        Self {
            db_timeout: Duration::from_secs(2),
            redis_timeout: Duration::from_secs(2),
        }
    }
}

/// Status of service health checks
#[derive(Debug, serde::Serialize)]
struct ServiceStatus {
    healthy: bool,
    latency_ms: u128,
    error: Option<String>,
}

/// Detailed health check response
#[derive(Debug, serde::Serialize)]
struct HealthResponse {
    status: String,
    version: String,
    environment: String,
    database: ServiceStatus,
    redis: ServiceStatus,
}

/// Performs a comprehensive health check of the application
#[instrument(skip(state))]
pub async fn health_check(
    State(state): State<Arc<AppState>>,
) -> Json<Value> {
    let config = HealthCheckConfig::default();
    
    // Run health checks concurrently
    let (db_result, redis_result) = tokio::join!(
        check_database(&state.db_pool, config.db_timeout),
        check_redis(&state.redis_client, config.redis_timeout)
    );

    let overall_status = if db_result.healthy && redis_result.healthy {
        "healthy"
    } else {
        "unhealthy"
    };

    let response = HealthResponse {
        status: overall_status.to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        environment: state.config.environment.clone(),
        database: db_result,
        redis: redis_result,
    };

    trace!(?response, "Health check completed");
    
    Json(json!(response))
}

/// Checks database connection health
#[instrument(skip(pool))]
async fn check_database(pool: &DbPool, timeout: Duration) -> ServiceStatus {
    let start = Instant::now();
    
    let result = tokio::time::timeout(timeout, async {
        pool.get().map(|_| true)
    }).await;

    let latency = start.elapsed().as_millis();
    
    match result {
        Ok(Ok(_)) => ServiceStatus {
            healthy: true,
            latency_ms: latency,
            error: None,
        },
        Ok(Err(e)) => {
            warn!(error = %e, "Database connection failed");
            ServiceStatus {
                healthy: false,
                latency_ms: latency,
                error: Some(e.to_string()),
            }
        }
        Err(_) => {
            warn!("Database check timed out");
            ServiceStatus {
                healthy: false,
                latency_ms: latency,
                error: Some(format!("Timeout after {}ms", timeout.as_millis())),
            }
        }
    }
}

/// Checks Redis connection health
#[instrument(skip(client))]
async fn check_redis(client: &RedisClient, timeout: Duration) -> ServiceStatus {
    let start = Instant::now();
    
    let result = tokio::time::timeout(timeout, async {
        let mut conn = client.get_async_connection().await?;
        conn.ping::<String>().await.map(|_| true)
    }).await;

    let latency = start.elapsed().as_millis();
    
    match result {
        Ok(Ok(_)) => ServiceStatus {
            healthy: true,
            latency_ms: latency,
            error: None,
        },
        Ok(Err(e)) => {
            warn!(error = %e, "Redis connection failed");
            ServiceStatus {
                healthy: false,
                latency_ms: latency,
                error: Some(e.to_string()),
            }
        }
        Err(_) => {
            warn!("Redis check timed out");
            ServiceStatus {
                healthy: false,
                latency_ms: latency,
                error: Some(format!("Timeout after {}ms", timeout.as_millis())),
            }
        }
    }
}

/// Creates a router with the health check endpoint
pub fn health_check_route() -> Router<Arc<AppState>> {
    Router::new()
        .route("/health", get(health_check))
        .route("/healthz", get(health_check)) // Common alternative endpoint
}

/// Health check error types
#[derive(Debug, thiserror::Error)]
pub enum HealthCheckError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("Redis error: {0}")]
    Redis(#[from] redis::RedisError),
    #[error("Timeout after {0:?}")]
    Timeout(Duration),
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_health_check() {
        // Mock AppState setup would go here
        let app = health_check_route(); // Add appropriate state
        
        let response = app
            .oneshot(
                axum::http::Request::builder()
                    .uri("/health")
                    .body(axum::body::Body::empty())
                    .unwrap()
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }
}