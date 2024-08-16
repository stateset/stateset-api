use axum::{
    routing::post,
    extract::{State, Json},
    response::IntoResponse,
    Router,
};
use crate::db::DbPool;
use crate::models::user::{NewUser, User, LoginCredentials};
use crate::errors::ServiceError;
use crate::services::auth::{register_user, login_user, refresh_user_token};
use validator::Validate;
use std::sync::Arc;

async fn register(
    State(pool): State<Arc<DbPool>>,
    Json(user_info): Json<NewUser>,
) -> Result<impl IntoResponse, ServiceError> {
    user_info.validate()?;
    let created_user = register_user(&pool, user_info).await?;
    Ok((axum::http::StatusCode::CREATED, Json(created_user)))
}

async fn login(
    State(pool): State<Arc<DbPool>>,
    Json(credentials): Json<LoginCredentials>,
) -> Result<impl IntoResponse, ServiceError> {
    let tokens = login_user(&pool, credentials).await?;
    Ok(Json(tokens))
}

async fn refresh_token(
    State(pool): State<Arc<DbPool>>,
    Json(refresh_token): Json<String>,
) -> Result<impl IntoResponse, ServiceError> {
    let new_tokens = refresh_user_token(&pool, refresh_token).await?;
    Ok(Json(new_tokens))
}

pub fn auth_routes() -> Router {
    Router::new()
        .route("/register", post(register))
        .route("/login", post(login))
        .route("/refresh", post(refresh_token))
}