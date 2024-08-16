use axum::{
    extract::State,
    routing::post,
    Json, Router,
};
use serde_json::json;
use crate::db::DbPool;
use crate::models::{NewUser, User, LoginCredentials};
use crate::errors::ServiceError;
use crate::auth;
use crate::config::AppConfig;
use validator::Validate;

async fn register(
    State(pool): State<DbPool>,
    State(config): State<AppConfig>,
    Json(user_info): Json<NewUser>,
) -> Result<Json<String>, ServiceError> {
    user_info.validate()?;
    Ok(Json("User registered successfully".to_string()))
}

async fn login(
    State(pool): State<DbPool>,
    State(config): State<AppConfig>,
    Json(credentials): Json<LoginCredentials>,
) -> Result<Json<serde_json::Value>, ServiceError> {
    // Implement login logic
    // Verify password hash
    let token = auth::create_jwt(&credentials.username, &config.jwt_secret);
    Ok(Json(json!({ "token": token })))
}

pub fn auth_routes() -> Router<(DbPool, AppConfig)> {
    Router::new()
        .route("/register", post(register))
        .route("/login", post(login))
}