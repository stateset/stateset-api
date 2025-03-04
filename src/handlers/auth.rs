use axum::{
    routing::{post, get},
    extract::{State, Json},
    Router,
    http::{StatusCode, Request, Response},
    middleware::Next,
};
use std::sync::Arc;
use crate::{
    errors::ApiError,
    AppState,
};
use serde::{Serialize, Deserialize};
use validator::Validate;
use tracing::info;
use super::common::{validate_input, success_response, created_response};
use jsonwebtoken::{encode, decode, Header, EncodingKey, DecodingKey, Validation, Algorithm};
use chrono::{Utc, Duration};
use uuid::Uuid;

// Request and response DTOs

#[derive(Debug, Deserialize, Validate)]
pub struct LoginRequest {
    #[validate(email)]
    pub email: String,
    #[validate(length(min = 6, message = "Password must be at least 6 characters"))]
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: i64,
    pub token_type: String,
}

#[derive(Debug, Deserialize, Validate)]
pub struct RefreshTokenRequest {
    pub refresh_token: String,
}

#[derive(Debug, Deserialize, Validate)]
pub struct RegisterRequest {
    #[validate(length(min = 2, message = "Name must be at least 2 characters"))]
    pub name: String,
    #[validate(email)]
    pub email: String,
    #[validate(length(min = 6, message = "Password must be at least 6 characters"))]
    pub password: String,
}

// JWT claims structure
#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    sub: String,         // Subject (user ID)
    name: String,        // User's name
    email: String,       // User's email
    role: String,        // User's role
    exp: i64,            // Expiration time
    iat: i64,            // Issued at time
    nbf: i64,            // Not valid before time
}

// Handler functions

/// Login handler
async fn login(
    State(state): State<Arc<AppState>>,
    Json(login_req): Json<LoginRequest>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    validate_input(&login_req)?;
    
    // This is a placeholder for the actual authentication logic
    // In a real implementation, you would:
    // 1. Check the user's credentials against the database
    // 2. Generate JWT tokens if credentials are valid
    
    // For this example, we'll always succeed with dummy data
    let user_id = "00000000-0000-0000-0000-000000000001".to_string();
    let user_name = "John Doe".to_string();
    
    let now = Utc::now();
    let jwt_expiration = (now + Duration::minutes(state.config.jwt_expiration as i64)).timestamp();
    
    let claims = Claims {
        sub: user_id,
        name: user_name,
        email: login_req.email,
        role: "user".to_string(),
        exp: jwt_expiration,
        iat: now.timestamp(),
        nbf: now.timestamp(),
    };
    
    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(state.config.jwt_secret.as_bytes()),
    ).map_err(|_| ApiError::InternalServerError("Failed to generate token".to_string()))?;
    
    // Generate a refresh token (simple UUID in this example)
    let refresh_token = Uuid::new_v4().to_string();
    
    // In a real implementation, you would:
    // 1. Hash the refresh token
    // 2. Store it in the database with expiration time
    // 3. Associate it with the user
    
    info!("User logged in: {}", claims.email);
    
    success_response(TokenResponse {
        access_token: token,
        refresh_token,
        expires_in: state.config.jwt_expiration as i64,
        token_type: "bearer".to_string(),
    })
}

/// Refresh token handler
async fn refresh_token(
    State(state): State<Arc<AppState>>,
    Json(refresh_req): Json<RefreshTokenRequest>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    // This is a placeholder for the actual refresh token logic
    // In a real implementation, you would:
    // 1. Validate the refresh token against the database
    // 2. Generate new JWT tokens if the refresh token is valid
    
    // For this example, we'll always succeed with dummy data
    let user_id = "00000000-0000-0000-0000-000000000001".to_string();
    let user_name = "John Doe".to_string();
    let user_email = "john.doe@example.com".to_string();
    
    let now = Utc::now();
    let jwt_expiration = (now + Duration::minutes(state.config.jwt_expiration as i64)).timestamp();
    
    let claims = Claims {
        sub: user_id,
        name: user_name,
        email: user_email,
        role: "user".to_string(),
        exp: jwt_expiration,
        iat: now.timestamp(),
        nbf: now.timestamp(),
    };
    
    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(state.config.jwt_secret.as_bytes()),
    ).map_err(|_| ApiError::InternalServerError("Failed to generate token".to_string()))?;
    
    // Generate a new refresh token
    let new_refresh_token = Uuid::new_v4().to_string();
    
    info!("Tokens refreshed for user: {}", claims.email);
    
    success_response(TokenResponse {
        access_token: token,
        refresh_token: new_refresh_token,
        expires_in: state.config.jwt_expiration as i64,
        token_type: "bearer".to_string(),
    })
}

/// Register a new user
async fn register(
    State(state): State<Arc<AppState>>,
    Json(register_req): Json<RegisterRequest>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    validate_input(&register_req)?;
    
    // This is a placeholder for the actual registration logic
    // In a real implementation, you would:
    // 1. Check if the user already exists
    // 2. Hash the password
    // 3. Store the user in the database
    // 4. Generate JWT tokens
    
    // For this example, we'll always succeed
    let user_id = Uuid::new_v4().to_string();
    
    // Generate token as in login
    let now = Utc::now();
    let jwt_expiration = (now + Duration::minutes(state.config.jwt_expiration as i64)).timestamp();
    
    let claims = Claims {
        sub: user_id,
        name: register_req.name.clone(),
        email: register_req.email.clone(),
        role: "user".to_string(),
        exp: jwt_expiration,
        iat: now.timestamp(),
        nbf: now.timestamp(),
    };
    
    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(state.config.jwt_secret.as_bytes()),
    ).map_err(|_| ApiError::InternalServerError("Failed to generate token".to_string()))?;
    
    // Generate a refresh token
    let refresh_token = Uuid::new_v4().to_string();
    
    info!("User registered: {}", register_req.email);
    
    created_response(TokenResponse {
        access_token: token,
        refresh_token,
        expires_in: state.config.jwt_expiration as i64,
        token_type: "bearer".to_string(),
    })
}

/// Creates the router for authentication endpoints
pub fn auth_routes() -> Router {
    Router::new()
        .route("/login", post(login))
        .route("/refresh", post(refresh_token))
        .route("/register", post(register))
}

/// Authentication middleware that checks if the request has a valid JWT token
pub async fn auth_middleware<B>(
    request: Request<B>,
    next: Next<B>,
) -> Result<Response, StatusCode> {
    // Skip authentication for login, refresh, and register endpoints
    let path = request.uri().path();
    if path.starts_with("/api/auth/") || path == "/health" || path == "/api/health" {
        return Ok(next.run(request).await);
    }
    
    // Check if the request has an Authorization header
    let auth_header = request
        .headers()
        .get("Authorization")
        .and_then(|header| header.to_str().ok())
        .ok_or(StatusCode::UNAUTHORIZED)?;
    
    // Extract the token from the Authorization header
    let token = auth_header
        .strip_prefix("Bearer ")
        .ok_or(StatusCode::UNAUTHORIZED)?;
    
    // Validate the token
    // In a real implementation, you would:
    // 1. Decode the token
    // 2. Validate the token against the JWT secret
    // 3. Attach the authenticated user to the request extensions
    
    // For this example, we'll always succeed
    // In a real implementation, you would use the token to fetch the user and attach it to the request
    
    Ok(next.run(request).await)
}
