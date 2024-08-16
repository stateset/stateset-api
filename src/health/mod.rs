use axum::{
    extract::State,
    routing::get,
    Json, Router,
};
use serde_json::json;
use crate::AppState;
use redis::{AsyncCommands, RedisResult};
use std::sync::Arc;

pub async fn health_check(
    State(state): State<Arc<AppState>>,
) -> Json<serde_json::Value> {
    let db_status = check_database(&state.db_pool).await;
    let redis_status = check_redis(&state.redis_client).await;

    if db_status && redis_status {
        Json(json!({
            "status": "healthy",
            "version": env!("CARGO_PKG_VERSION"),
            "environment": &state.config.environment
        }))
    } else {
        Json(json!({
            "status": "unhealthy",
            "database": db_status,
            "redis": redis_status
        }))
    }
}

async fn check_database(pool: &DbPool) -> bool {
    pool.get().is_ok()
}

async fn check_redis(client: &RedisClient) -> bool {
    match client.get_async_connection().await {
        Ok(mut conn) => {
            let result: RedisResult<String> = conn.ping().await;
            result.is_ok()
        }
        Err(_) => false,
    }
}

pub fn health_check_route() -> Router<Arc<AppState>> {
    Router::new().route("/health", get(health_check))
}