use actix_web::{post, web, HttpResponse};
use crate::db::DbPool;
use crate::models::user::{NewUser, User, LoginCredentials};
use crate::errors::ServiceError;
use crate::services::auth::{register_user, login_user, refresh_user_token};
use validator::Validate;

#[post("/register")]
async fn register(
    pool: web::Data<DbPool>,
    user_info: web::Json<NewUser>,
) -> Result<HttpResponse, ServiceError> {
    user_info.validate()?;
    let created_user = register_user(&pool, user_info.into_inner()).await?;
    Ok(HttpResponse::Created().json(created_user))
}

#[post("/login")]
async fn login(
    pool: web::Data<DbPool>,
    credentials: web::Json<LoginCredentials>,
) -> Result<HttpResponse, ServiceError> {
    let tokens = login_user(&pool, credentials.into_inner()).await?;
    Ok(HttpResponse::Ok().json(tokens))
}

#[post("/refresh")]
async fn refresh_token(
    pool: web::Data<DbPool>,
    refresh_token: web::Json<String>,
) -> Result<HttpResponse, ServiceError> {
    let new_tokens = refresh_user_token(&pool, refresh_token.into_inner()).await?;
    Ok(HttpResponse::Ok().json(new_tokens))
}