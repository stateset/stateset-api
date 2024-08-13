use actix_web::{web, HttpResponse};
use crate::AppState;

pub async fn health_check(state: web::Data<AppState>) -> HttpResponse {
    let db_status = check_database(&state.db_pool).await;
    let redis_status = check_redis(&state.redis_client).await;

    if db_status && redis_status {
        HttpResponse::Ok().json(serde_json::json!({
            "status": "healthy",
            "version": env!("CARGO_PKG_VERSION"),
            "environment": &state.config.environment
        }))
    } else {
        HttpResponse::ServiceUnavailable().json(serde_json::json!({
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
    let mut conn = match client.get_async_connection().await {
        Ok(conn) => conn,
        Err(_) => return false,
    };

    redis::cmd("PING").query_async::<_, String>(&mut conn).await.is_ok()
}