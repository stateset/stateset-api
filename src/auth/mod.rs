/*!
 * # Authentication and Authorization Module
 *
 * This module provides authentication and authorization services for the Stateset API.
 * It supports multiple authentication methods:
 *
 * - JWT (JSON Web Tokens) with refresh token support
 * - API Keys for service-to-service authentication
 * - OAuth2 integration (future expansion)
 *
 * The module also provides role-based access control (RBAC) and permission verification.
 */

use async_trait::async_trait;
use axum::{
    extract::{Request, State},
    http::{header, HeaderMap, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
    extract::DefaultBodyLimit,
};
use base64::Engine as _;
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use hmac::Mac;
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};
use sea_orm::{
    ColumnTrait, DatabaseConnection, EntityTrait, ModelTrait,
};
use serde::{Deserialize, Serialize};
use sha2::Digest;
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tokio::sync::RwLock;
use tracing::{debug, error};
use uuid::Uuid;

// Entity modules
pub mod api_key;
pub mod api_key_permission;
pub mod refresh_token;
pub mod user;
pub mod user_role;

// Feature modules
mod api_key_service;
mod permissions;
mod rate_limit;
mod rbac;
mod types;

// Re-exports
pub use api_key_service::*;
pub use permissions::*;
pub use rate_limit::*;
pub use rbac::*;
pub use types::*;

/// Claim structure for JWT tokens
#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,               // Subject (user ID)
    pub name: Option<String>,      // User's name
    pub email: Option<String>,     // User's email
    pub roles: Vec<String>,        // User's roles (multiple roles support)
    pub permissions: Vec<String>,  // User's explicit permissions
    pub tenant_id: Option<String>, // For multi-tenant support
    pub jti: String,               // JWT ID (unique identifier for this token)
    pub iat: i64,                  // Issued at time
    pub exp: i64,                  // Expiration time
    pub nbf: i64,                  // Not valid before time
    pub iss: String,               // Issuer
    pub aud: String,               // Audience
    pub scope: Option<String>,     // OAuth2 scopes
}

/// Authenticated user data extracted from the JWT token
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthUser {
    pub user_id: String,
    pub name: Option<String>,
    pub email: Option<String>,
    pub roles: Vec<String>,
    pub permissions: Vec<String>,
    pub tenant_id: Option<String>,
    pub token_id: String,
    pub is_api_key: bool,
}

impl AuthUser {
    /// Check if the user has a specific role
    pub fn has_role(&self, role: &str) -> bool {
        self.roles.iter().any(|r| r == role)
    }

    /// Check if the user has a specific permission
    pub fn has_permission(&self, permission: &str) -> bool {
        self.permissions.iter().any(|p| p == permission)
    }

    /// Check if the user belongs to a specific tenant
    pub fn belongs_to_tenant(&self, tenant_id: &str) -> bool {
        self.tenant_id
            .as_ref()
            .map_or(false, |tid| tid == tenant_id)
    }

    /// Check if the user is an admin
    pub fn is_admin(&self) -> bool {
        self.has_role("admin")
    }
}

/// Authentication configuration
#[derive(Clone, Debug)]
pub struct AuthConfig {
    pub jwt_secret: String,
    pub jwt_audience: String,
    pub jwt_issuer: String,
    pub access_token_expiration: Duration,
    pub refresh_token_expiration: Duration,
    pub api_key_prefix: String,
}

impl AuthConfig {
    pub fn new(
        jwt_secret: String,
        jwt_audience: String,
        jwt_issuer: String,
        access_token_expiration: Duration,
        refresh_token_expiration: Duration,
        api_key_prefix: String,
    ) -> Self {
        Self {
            jwt_secret,
            jwt_audience,
            jwt_issuer,
            access_token_expiration,
            refresh_token_expiration,
            api_key_prefix,
        }
    }

    /// Create default configuration with secure defaults
    pub fn default() -> Self {
        Self {
            jwt_secret: "your-secret-key".to_string(), // Never use this in production
            jwt_audience: "stateset-api".to_string(),
            jwt_issuer: "stateset-auth".to_string(),
            access_token_expiration: Duration::from_secs(30 * 60), // 30 minutes
            refresh_token_expiration: Duration::from_secs(7 * 24 * 60 * 60), // 7 days
            api_key_prefix: "sk_".to_string(),
        }
    }
}

/// Authentication service that handles token issuance and validation
#[derive(Debug, Clone)]
pub struct AuthService {
    pub config: AuthConfig,
    pub db: Arc<DatabaseConnection>,
    pub blacklisted_tokens: Arc<RwLock<Vec<BlacklistedToken>>>,
}

/// Token blacklist entry
#[derive(Clone, Debug)]
struct BlacklistedToken {
    jti: String,
    expiry: DateTime<Utc>,
}

impl AuthService {
    /// Create a new authentication service
    pub fn new(config: AuthConfig, db: Arc<DatabaseConnection>) -> Self {
        Self {
            config,
            db,
            blacklisted_tokens: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Generate a JWT token for a user
    pub async fn generate_token(&self, user: &User) -> Result<TokenPair, AuthError> {
        let now = Utc::now();
        let access_exp = now
            + ChronoDuration::from_std(self.config.access_token_expiration)
                .map_err(|_| AuthError::InternalError("Invalid token duration".to_string()))?;
        let refresh_exp = now
            + ChronoDuration::from_std(self.config.refresh_token_expiration)
                .map_err(|_| AuthError::InternalError("Invalid token duration".to_string()))?;

        // Generate unique token IDs
        let access_jti = Uuid::new_v4().to_string();
        let refresh_jti = Uuid::new_v4().to_string();

        // Get user roles and permissions
        let roles = self.get_user_roles(user.id).await?;
        let permissions = self.get_user_permissions(user.id).await?;

        // Create access token claims
        let access_claims = Claims {
            sub: user.id.to_string(),
            name: Some(user.name.clone()),
            email: Some(user.email.clone()),
            roles: roles.clone(),
            permissions: permissions.clone(),
            tenant_id: user.tenant_id.clone(),
            jti: access_jti.clone(),
            iat: now.timestamp(),
            exp: access_exp.timestamp(),
            nbf: now.timestamp(),
            iss: self.config.jwt_issuer.clone(),
            aud: self.config.jwt_audience.clone(),
            scope: None,
        };

        // Create refresh token claims (with minimal data)
        let refresh_claims = Claims {
            sub: user.id.to_string(),
            name: None,
            email: None,
            roles: vec![],
            permissions: vec![],
            tenant_id: user.tenant_id.clone(),
            jti: refresh_jti.clone(),
            iat: now.timestamp(),
            exp: refresh_exp.timestamp(),
            nbf: now.timestamp(),
            iss: self.config.jwt_issuer.clone(),
            aud: self.config.jwt_audience.clone(),
            scope: None,
        };

        // Generate the tokens
        let access_token = encode(
            &Header::new(Algorithm::HS256),
            &access_claims,
            &EncodingKey::from_secret(self.config.jwt_secret.as_bytes()),
        )
        .map_err(|e| AuthError::TokenCreation(e.to_string()))?;

        let refresh_token = encode(
            &Header::new(Algorithm::HS256),
            &refresh_claims,
            &EncodingKey::from_secret(self.config.jwt_secret.as_bytes()),
        )
        .map_err(|e| AuthError::TokenCreation(e.to_string()))?;

        // Store the refresh token
        self.store_refresh_token(user.id, &refresh_jti, refresh_exp)
            .await?;

        Ok(TokenPair {
            access_token,
            refresh_token,
            token_type: "Bearer".to_string(),
            expires_in: self.config.access_token_expiration.as_secs() as i64,
            refresh_expires_in: self.config.refresh_token_expiration.as_secs() as i64,
        })
    }

    /// Validate a JWT token and extract the claims
    pub async fn validate_token(&self, token: &str) -> Result<Claims, AuthError> {
        // Decode and validate the token
        let claims = decode::<Claims>(
            token,
            &DecodingKey::from_secret(self.config.jwt_secret.as_bytes()),
            &Validation::new(Algorithm::HS256),
        )
        .map_err(|e| match e.kind() {
            jsonwebtoken::errors::ErrorKind::ExpiredSignature => AuthError::TokenExpired,
            jsonwebtoken::errors::ErrorKind::InvalidToken => AuthError::InvalidToken,
            _ => AuthError::InvalidToken,
        })?
        .claims;

        // Check if the token is blacklisted
        if self.is_token_blacklisted(&claims.jti).await {
            return Err(AuthError::RevokedToken);
        }

        Ok(claims)
    }

    /// Refresh an access token using a refresh token
    pub async fn refresh_token(&self, refresh_token: &str) -> Result<TokenPair, AuthError> {
        // Validate the refresh token
        let claims = self.validate_token(refresh_token).await?;

        // Get the user ID from the claims
        let user_id = Uuid::parse_str(&claims.sub).map_err(|_| AuthError::InvalidToken)?;

        // Check if the refresh token exists in the database
        let refresh_token_exists = self.verify_refresh_token(user_id, &claims.jti).await?;
        if !refresh_token_exists {
            return Err(AuthError::InvalidToken);
        }

        // Get the user
        let user = self.get_user(user_id).await?;

        // Generate new tokens
        let new_tokens = self.generate_token(&user).await?;

        // Invalidate the old refresh token
        self.revoke_refresh_token(user_id, &claims.jti).await?;

        Ok(new_tokens)
    }

    /// Revoke a token (add it to the blacklist)
    pub async fn revoke_token(&self, token: &str) -> Result<(), AuthError> {
        // Validate the token first
        let claims = self.validate_token(token).await?;

        // Add the token to the blacklist
        let expiry = Utc::now() + ChronoDuration::seconds(claims.exp - Utc::now().timestamp());
        let blacklisted_token = BlacklistedToken {
            jti: claims.jti,
            expiry,
        };

        // Add to the in-memory blacklist
        let mut blacklist = self.blacklisted_tokens.write().await;
        blacklist.push(blacklisted_token);

        // Clean up expired tokens in the blacklist
        self.clean_blacklist(&mut blacklist);

        Ok(())
    }

    /// Check if a token is blacklisted
    async fn is_token_blacklisted(&self, token_id: &str) -> bool {
        let blacklist = self.blacklisted_tokens.read().await;
        blacklist.iter().any(|t| t.jti == token_id)
    }

    /// Clean up expired tokens from the blacklist
    fn clean_blacklist(&self, blacklist: &mut Vec<BlacklistedToken>) {
        let now = Utc::now();
        blacklist.retain(|t| t.expiry > now);
    }

    /// Get a user by ID
    async fn get_user(&self, user_id: Uuid) -> Result<User, AuthError> {
        // This would fetch the user from the database
        // For now, we'll just return a mock user
        Ok(User {
            id: user_id,
            name: "Test User".to_string(),
            email: "test@example.com".to_string(),
            password_hash: "".to_string(),
            tenant_id: Some("tenant1".to_string()),
            active: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        })
    }

    /// Get user roles
    async fn get_user_roles(&self, user_id: Uuid) -> Result<Vec<String>, AuthError> {
        // This would fetch the user's roles from the database
        // For now, we'll just return some mock roles
        Ok(vec!["user".to_string()])
    }

    /// Get user permissions
    async fn get_user_permissions(&self, user_id: Uuid) -> Result<Vec<String>, AuthError> {
        // This would fetch the user's permissions from the database
        // For now, we'll just return some mock permissions
        Ok(vec!["orders:read".to_string(), "orders:create".to_string()])
    }

    /// Store a refresh token
    async fn store_refresh_token(
        &self,
        user_id: Uuid,
        token_id: &str,
        expiry: DateTime<Utc>,
    ) -> Result<(), AuthError> {
        // This would store the refresh token in the database
        // For now, we'll just log it
        debug!("Stored refresh token: {} for user: {}", token_id, user_id);
        Ok(())
    }

    /// Verify a refresh token
    async fn verify_refresh_token(&self, user_id: Uuid, token_id: &str) -> Result<bool, AuthError> {
        // This would verify the refresh token in the database
        // For now, we'll just return true
        Ok(true)
    }

    /// Revoke a refresh token
    async fn revoke_refresh_token(&self, user_id: Uuid, token_id: &str) -> Result<(), AuthError> {
        // This would remove the refresh token from the database
        // For now, we'll just log it
        debug!("Revoked refresh token: {} for user: {}", token_id, user_id);
        Ok(())
    }

    /// Validate API key
    pub async fn validate_api_key(&self, api_key: &str) -> Result<ApiKey, AuthError> {
        // Check if the API key has the correct prefix
        if !api_key.starts_with(&self.config.api_key_prefix) {
            return Err(AuthError::InvalidApiKey);
        }

        // Here you would look up the API key in your database
        // For this example, we'll create a mock API key
        let api_key_info = ApiKey {
            id: Uuid::new_v4(),
            name: "Test API Key".to_string(),
            key: api_key.to_string(),
            user_id: Uuid::new_v4(),
            roles: vec!["api_user".to_string()],
            permissions: vec!["orders:read".to_string(), "orders:create".to_string()],
            tenant_id: Some("tenant1".to_string()),
            created_at: Utc::now(),
            expires_at: Some(Utc::now() + ChronoDuration::days(30)),
            last_used_at: Some(Utc::now()),
        };

        Ok(api_key_info)
    }

    /// Generate a secure API key
    pub fn generate_api_key(&self, name: &str) -> ApiKeyCreationResponse {
        // Generate a random string for the key
        let key_bytes: Vec<u8> = thread_rng().sample_iter(&Alphanumeric).take(32).collect();
        let key = String::from_utf8(key_bytes).unwrap();

        // Prefix the key
        let api_key = format!("{}{}", self.config.api_key_prefix, key);

        // In a real implementation, you would hash and store this key
        // and associate it with a user and permissions

        ApiKeyCreationResponse {
            api_key,
            name: name.to_string(),
            expires_at: Utc::now() + ChronoDuration::days(30),
        }
    }
}

/// Token pair response
#[derive(Debug, Serialize, Deserialize)]
pub struct TokenPair {
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: String,
    pub expires_in: i64,
    pub refresh_expires_in: i64,
}

/// Login credentials
#[derive(Debug, Serialize, Deserialize)]
pub struct LoginCredentials {
    pub email: String,
    pub password: String,
}

/// API key creation response
#[derive(Debug, Serialize, Deserialize)]
pub struct ApiKeyCreationResponse {
    pub api_key: String,
    pub name: String,
    pub expires_at: DateTime<Utc>,
}

/// User model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: Uuid,
    pub name: String,
    pub email: String,
    pub password_hash: String,
    pub tenant_id: Option<String>,
    pub active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Authentication error types
#[derive(Debug, Error)]
pub enum AuthError {
    #[error("Missing authentication")]
    MissingAuth,

    #[error("Invalid credentials")]
    InvalidCredentials,

    #[error("Missing token")]
    MissingToken,

    #[error("Invalid token")]
    InvalidToken,

    #[error("Token has expired")]
    TokenExpired,

    #[error("Token has been revoked")]
    RevokedToken,

    #[error("Token creation failed: {0}")]
    TokenCreation(String),

    #[error("Invalid API key")]
    InvalidApiKey,

    #[error("Expired API key")]
    ExpiredApiKey,

    #[error("Invalid API key signature")]
    InvalidApiKeySignature,

    #[error("User not found")]
    UserNotFound,

    #[error("Insufficient permissions")]
    InsufficientPermissions,

    #[error("Database error: {0}")]
    DatabaseError(String),

    #[error("Internal error: {0}")]
    InternalError(String),
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        let (status, error_code, error_message): (StatusCode, &str, String) = match &self {
            Self::MissingAuth => (
                StatusCode::UNAUTHORIZED,
                "AUTH_MISSING",
                "Authentication required".to_string(),
            ),
            Self::InvalidCredentials => (
                StatusCode::UNAUTHORIZED,
                "AUTH_INVALID_CREDENTIALS",
                "Invalid credentials".to_string(),
            ),
            Self::MissingToken => (
                StatusCode::UNAUTHORIZED,
                "AUTH_MISSING_TOKEN",
                "No authentication token provided".to_string(),
            ),
            Self::InvalidToken => (
                StatusCode::UNAUTHORIZED,
                "AUTH_INVALID_TOKEN",
                "Invalid authentication token".to_string(),
            ),
            Self::TokenExpired => (
                StatusCode::UNAUTHORIZED,
                "AUTH_TOKEN_EXPIRED",
                "Token has expired".to_string(),
            ),
            Self::RevokedToken => (
                StatusCode::UNAUTHORIZED,
                "AUTH_REVOKED_TOKEN",
                "Authentication token has been revoked".to_string(),
            ),
            Self::TokenCreation(msg) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "AUTH_TOKEN_CREATION_FAILED",
                msg.clone(),
            ),
            Self::InvalidApiKey => (
                StatusCode::UNAUTHORIZED,
                "AUTH_INVALID_API_KEY",
                "Invalid API key".to_string(),
            ),
            Self::ExpiredApiKey => (
                StatusCode::UNAUTHORIZED,
                "AUTH_EXPIRED_API_KEY",
                "API key has expired".to_string(),
            ),
            Self::InvalidApiKeySignature => (
                StatusCode::UNAUTHORIZED,
                "AUTH_INVALID_API_KEY_SIGNATURE",
                "Invalid API key signature".to_string(),
            ),
            Self::UserNotFound => (
                StatusCode::NOT_FOUND,
                "AUTH_USER_NOT_FOUND",
                "User not found".to_string(),
            ),
            Self::InsufficientPermissions => (
                StatusCode::FORBIDDEN,
                "AUTH_INSUFFICIENT_PERMISSIONS",
                "Insufficient permissions".to_string(),
            ),
            Self::DatabaseError(msg) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "AUTH_DATABASE_ERROR",
                msg.clone(),
            ),
            Self::InternalError(msg) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "AUTH_INTERNAL_ERROR",
                msg.clone(),
            ),
        };

        let body = Json(serde_json::json!({
            "error": {
                "code": error_code,
                "message": error_message,
            }
        }));

        (status, body).into_response()
    }
}

/// Extract authentication from a request (either JWT or API key)
// TODO: Fix lifetime issues with FromRequestParts implementation
// Commenting out for now to get basic compilation working
/*
#[async_trait]
impl<S> FromRequestParts<S> for AuthUser
where
    S: Send + Sync,
    Arc<AuthService>: FromRequestParts<S>,
{
    type Rejection = AuthError;

    async fn from_request_parts<'life0, 'life1, 'async_trait>(
        parts: &'life0 mut axum::http::request::Parts,
        state: &'life1 S,
    ) -> core::pin::Pin<Box<dyn core::future::Future<Output = Result<Self, Self::Rejection>> + core::marker::Send + 'async_trait>>
    where
        'life0: 'async_trait,
        'life1: 'async_trait,
        Self: 'async_trait,
    {
        Box::pin(async move {
        // Extract auth service from the state
        let auth_service = Arc::<AuthService>::from_request_parts(parts, state)
            .await
            .map_err(|_| AuthError::InternalError("Failed to extract auth service".to_string()))?;

        // Try JWT first (from Authorization header)
        if let Some(auth_header) = parts.headers.get(header::AUTHORIZATION) {
            if let Ok(auth_value) = auth_header.to_str() {
                if auth_value.starts_with("Bearer ") {
                    let token = auth_value.trim_start_matches("Bearer ").trim();
                    let claims = auth_service.validate_token(token).await?;

                    return Ok(AuthUser {
                        user_id: claims.sub,
                        name: claims.name,
                        email: claims.email,
                        roles: claims.roles,
                        permissions: claims.permissions,
                        tenant_id: claims.tenant_id,
                        token_id: claims.jti,
                        is_api_key: false,
                    });
                }
            }
        }

        // Try API key (from X-API-Key header)
        if let Some(api_key_header) = parts.headers.get("X-API-Key") {
            if let Ok(api_key) = api_key_header.to_str() {
                let api_key_info = auth_service.validate_api_key(api_key).await?;

                // Check if the API key has expired
                if let Some(expires_at) = api_key_info.expires_at {
                    if expires_at < Utc::now() {
                        return Err(AuthError::ExpiredApiKey);
                    }
                }

                return Ok(AuthUser {
                    user_id: api_key_info.user_id.to_string(),
                    name: Some(api_key_info.name),
                    email: None,
                    roles: api_key_info.roles,
                    permissions: api_key_info.permissions,
                    tenant_id: api_key_info.tenant_id,
                    token_id: api_key_info.id.to_string(),
                    is_api_key: true,
                });
            }
        }

        // No valid authentication found
        Err(AuthError::MissingAuth)
        })
    }
}
*/

/// Permission requirement for endpoints
#[derive(Clone, Debug)]
pub struct RequirePermission(pub String);

/*
#[async_trait]
impl<S> FromRequestParts<S> for RequirePermission
where
    S: Send + Sync,
    AuthUser: FromRequestParts<S>,
{
    type Rejection = AuthError;

    async fn from_request_parts<'life0, 'life1, 'async_trait>(
        _parts: &'life0 mut axum::http::request::Parts,
        _state: &'life1 S,
    ) -> core::pin::Pin<Box<dyn core::future::Future<Output = Result<Self, Self::Rejection>> + core::marker::Send + 'async_trait>>
    where
        'life0: 'async_trait,
        'life1: 'async_trait,
        Self: 'async_trait,
    {
        Box::pin(async move {
            // This is just an extractor that returns itself
            // The actual permission check is done in the middleware
            Ok(Self("".to_string()))
        })
    }
}
*/

/// Permission middleware to check if a user has the required permission
pub async fn permission_middleware(
    State(required_permission): State<String>,
    mut request: Request,
    next: Next,
) -> Result<Response, AuthError> {
    // Extract the authenticated user
    let user = match request.extensions().get::<AuthUser>() {
        Some(user) => user.clone(),
        None => return Err(AuthError::MissingAuth),
    };

    // Check if the user has admin role (admins have all permissions)
    if user.has_role("admin") {
        return Ok(next.run(request).await);
    }

    // Check if the user has the required permission
    if !user.has_permission(&required_permission) {
        return Err(AuthError::InsufficientPermissions);
    }

    // User has the required permission, proceed with the request
    Ok(next.run(request).await)
}

/// Role middleware to check if a user has the required role
pub async fn role_middleware(
    State(required_role): State<String>,
    mut request: Request,
    next: Next,
) -> Result<Response, AuthError> {
    // Extract the authenticated user
    let user = match request.extensions().get::<AuthUser>() {
        Some(user) => user.clone(),
        None => return Err(AuthError::MissingAuth),
    };

    // Check if the user has the required role
    if !user.has_role(&required_role) {
        return Err(AuthError::InsufficientPermissions);
    }

    // User has the required role, proceed with the request
    Ok(next.run(request).await)
}

/// Authentication middleware that extracts and validates auth tokens
pub async fn auth_middleware(mut request: Request, next: Next) -> Response {
    // Clone the headers to avoid borrowing issues
    let headers = request.headers().clone();

    // Extract the auth service from the request state
    let auth_service = match request.extensions().get::<Arc<AuthService>>() {
        Some(service) => service.clone(),
        None => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Authentication service not available",
            )
                .into_response();
        }
    };

    // Extract auth information
    let auth_result = extract_auth_from_headers(&headers, &auth_service).await;

    match auth_result {
        Ok(user) => {
            // Add the authenticated user to the request extensions
            request.extensions_mut().insert(user);

            // Continue with the request
            next.run(request).await
        }
        Err(e) => e.into_response(),
    }
}

/// Extract authentication info from request headers
async fn extract_auth_from_headers(
    headers: &HeaderMap,
    auth_service: &AuthService,
) -> Result<AuthUser, AuthError> {
    // Try JWT first
    if let Some(auth_header) = headers.get(header::AUTHORIZATION) {
        if let Ok(auth_value) = auth_header.to_str() {
            if auth_value.starts_with("Bearer ") {
                let token = auth_value.trim_start_matches("Bearer ").trim();
                let claims = auth_service.validate_token(token).await?;

                return Ok(AuthUser {
                    user_id: claims.sub,
                    name: claims.name,
                    email: claims.email,
                    roles: claims.roles,
                    permissions: claims.permissions,
                    tenant_id: claims.tenant_id,
                    token_id: claims.jti,
                    is_api_key: false,
                });
            }
        }
    }

    // Try API key
    if let Some(api_key_header) = headers.get("X-API-Key") {
        if let Ok(api_key) = api_key_header.to_str() {
            let api_key_info = auth_service.validate_api_key(api_key).await?;

            // Check if the API key has expired
            if let Some(expires_at) = api_key_info.expires_at {
                if expires_at < Utc::now() {
                    return Err(AuthError::ExpiredApiKey);
                }
            }

            return Ok(AuthUser {
                user_id: api_key_info.user_id.to_string(),
                name: Some(api_key_info.name),
                email: None,
                roles: api_key_info.roles,
                permissions: api_key_info.permissions,
                tenant_id: api_key_info.tenant_id,
                token_id: api_key_info.id.to_string(),
                is_api_key: true,
            });
        }
    }

    // No valid authentication found
    Err(AuthError::MissingAuth)
}

/// Authentication routes
pub fn auth_routes() -> axum::Router<Arc<AuthService>> {
    axum::Router::new()
        .route("/login", axum::routing::post(login_handler))
        .route("/refresh", axum::routing::post(refresh_token_handler))
        // TODO: Fix handler trait compatibility
        // .route("/logout", axum::routing::post(logout_handler))
        // .route("/api-keys", axum::routing::post(create_api_key_handler))
        .layer(DefaultBodyLimit::max(1024 * 64)) // 64KB limit
}

/// Login handler
pub async fn login_handler(
    State(auth_service): State<Arc<AuthService>>,
    Json(credentials): Json<LoginCredentials>,
) -> Result<Json<TokenPair>, AuthError> {
    // In a real implementation, you would validate the credentials
    // against your database and get the user

    // For now, just create a mock user
    let user = User {
        id: Uuid::new_v4(),
        name: "Test User".to_string(),
        email: credentials.email,
        password_hash: "".to_string(),
        tenant_id: Some("tenant1".to_string()),
        active: true,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    // Generate tokens for the user
    let token_pair = auth_service.generate_token(&user).await?;

    Ok(Json(token_pair))
}

/// Refresh token handler
pub async fn refresh_token_handler(
    State(auth_service): State<Arc<AuthService>>,
    Json(refresh_request): Json<RefreshTokenRequest>,
) -> Result<Json<TokenPair>, AuthError> {
    // Refresh the token
    let token_pair = auth_service
        .refresh_token(&refresh_request.refresh_token)
        .await?;

    Ok(Json(token_pair))
}

/// Logout handler
async fn logout_handler(
    State(auth_service): State<Arc<AuthService>>,
    auth_user: AuthUser,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, AuthError> {
    // Extract the token from headers
    if let Some(auth_header) = headers.get(header::AUTHORIZATION) {
        if let Ok(auth_value) = auth_header.to_str() {
            if auth_value.starts_with("Bearer ") {
                let token = auth_value.trim_start_matches("Bearer ").trim();

                // Revoke the token
                auth_service.revoke_token(token).await?;
                return Ok(Json(serde_json::json!({ "message": "Successfully logged out" })));
            }
        }
    }

    Err(AuthError::MissingToken)
}

/// Create API key handler
async fn create_api_key_handler(
    State(auth_service): State<Arc<AuthService>>,
    auth_user: AuthUser,
    Json(request): Json<CreateApiKeyRequest>,
) -> Result<Json<ApiKeyCreationResponse>, AuthError> {
    // Check if the user has permission to create API keys
    if !auth_user.has_permission("api-keys:create") {
        return Err(AuthError::InsufficientPermissions);
    }

    // Generate a new API key
    let api_key_response = auth_service.generate_api_key(&request.name);

    Ok(Json(api_key_response))
}

/// Refresh token request
#[derive(Debug, Serialize, Deserialize)]
pub struct RefreshTokenRequest {
    pub refresh_token: String,
}

/// Create API key request
#[derive(Debug, Serialize, Deserialize)]
pub struct CreateApiKeyRequest {
    pub name: String,
}

/// Type alias for backwards compatibility
pub type AuthenticatedUser = AuthUser;

/// Extension methods for Router to add auth middleware
pub trait AuthRouterExt {
    fn with_auth(self) -> Self;
    fn with_permission(self, permission: &str) -> Self;
    fn with_role(self, role: &str) -> Self;
}

impl<S> AuthRouterExt for axum::Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    fn with_auth(self) -> Self {
        self.layer(axum::middleware::from_fn(auth_middleware))
    }

    fn with_permission(self, permission: &str) -> Self {
        self.layer(axum::middleware::from_fn_with_state(
            permission.to_string(),
            permission_middleware,
        ))
        .with_auth()
    }

    fn with_role(self, role: &str) -> Self {
        self.layer(axum::middleware::from_fn_with_state(
            role.to_string(),
            role_middleware,
        ))
        .with_auth()
    }
}


