use actix_web::{post, web, HttpResponse};
use crate::db::DbPool;
use crate::models::{NewUser, User};
use crate::errors::ServiceError;
use crate::auth;
use crate::config::AppConfig;
use validator::Validate;

#[post("/register")]
async fn register(
    pool: web::Data<DbPool>,
    config: web::Data<AppConfig>,
    user_info: web::Json<NewUser>
) -> Result<HttpResponse, ServiceError> {
    user_info.validate()?;
    // Implement user registration logic using diesel
    // Hash the password before storing
    Ok(HttpResponse::Ok().json("User registered successfully"))
}

#[post("/login")]
async fn login(
    pool: web::Data<DbPool>,
    config: web::Data<AppConfig>,
    credentials: web::Json<LoginCredentials>
) -> Result<HttpResponse, ServiceError> {
    // Implement login logic
    // Verify password hash
    let token = auth::create_jwt(&credentials.username, &config.jwt_secret);
    Ok(HttpResponse::Ok().json(json!({ "token": token })))
}