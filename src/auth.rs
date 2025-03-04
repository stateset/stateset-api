use axum::{
    async_trait,
    extract::{FromRequest, Request, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use jsonwebtoken::{decode, DecodingKey, Validation, Algorithm};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use std::sync::Arc;
use crate::AppState;

/// Claim structure for JWT tokens
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

/// Authenticated user data extracted from the JWT token
#[derive(Debug, Clone)]
pub struct AuthenticatedUser {
    pub user_id: String,
    pub name: String,
    pub email: String,
    pub role: String,
}

/// State of JWT
#[derive(Debug)]
pub struct JWTState {
    pub jwt_secret: String,
}

/// Extract the authenticated user from the request
#[async_trait]
impl<S> FromRequest<S> for AuthenticatedUser
where
    S: Send + Sync,
    Arc<AppState>: FromRequest<S>,
{
    type Rejection = AuthError;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        // Extract the JWT from the Authorization header
        let auth_header = req
            .headers()
            .get("Authorization")
            .ok_or(AuthError::MissingToken)?
            .to_str()
            .map_err(|_| AuthError::InvalidToken)?;
        
        // Check that it's a Bearer token
        let token = auth_header
            .strip_prefix("Bearer ")
            .ok_or(AuthError::InvalidToken)?;
        
        // Extract the AppState
        let app_state = Arc::<AppState>::from_request(req.clone(), state)
            .await
            .map_err(|_| AuthError::InternalError)?;
        
        // Decode and validate the token
        let claims = decode::<Claims>(
            token,
            &DecodingKey::from_secret(app_state.config.jwt_secret.as_bytes()),
            &Validation::new(Algorithm::HS256),
        )
        .map_err(|_| AuthError::InvalidToken)?
        .claims;
        
        // Check if the token is expired
        let now = chrono::Utc::now().timestamp();
        if claims.exp < now {
            return Err(AuthError::ExpiredToken);
        }
        
        // Return the authenticated user
        Ok(AuthenticatedUser {
            user_id: claims.sub,
            name: claims.name,
            email: claims.email,
            role: claims.role,
        })
    }
}

/// Authentication errors
#[derive(Debug)]
pub enum AuthError {
    MissingToken,
    InvalidToken,
    ExpiredToken,
    InternalError,
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            AuthError::MissingToken => (StatusCode::UNAUTHORIZED, "Missing token"),
            AuthError::InvalidToken => (StatusCode::UNAUTHORIZED, "Invalid token"),
            AuthError::ExpiredToken => (StatusCode::UNAUTHORIZED, "Token has expired"),
            AuthError::InternalError => (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error"),
        };
        
        let body = Json(serde_json::json!({
            "error": error_message,
        }));
        
        (status, body).into_response()
    }
}
