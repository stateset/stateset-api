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

use argon2::{
    password_hash::{PasswordHash, PasswordVerifier},
    Argon2,
};
use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum::{
    extract::DefaultBodyLimit,
    extract::{Request, State},
    http::{header, HeaderMap, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use metrics::counter;
use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};
use redis::AsyncCommands;
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};
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
pub mod oauth2;

// Re-exports
pub use api_key_service::*;
pub use permissions::*;
pub use rate_limit::*;
pub use rbac::*;
pub use types::*;
pub use oauth2::{OAuth2Config, OAuth2Provider, OAuth2ProviderConfig, OAuth2Service, OAuth2UserInfo};

use self::api_key::Entity as ApiKeyEntity;
use self::api_key_permission::Entity as ApiKeyPermissionEntity;
use self::refresh_token::Entity as RefreshTokenEntity;
use self::user::Entity as UserEntity;
use self::user_role::Entity as UserRoleEntity;

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

    /// Check if the user has a specific permission (admins have all permissions)
    pub fn has_permission(&self, permission: &str) -> bool {
        // Admins have all permissions
        if self.has_role("admin") {
            return true;
        }
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
    ) -> Result<Self, AuthError> {
        let config = Self {
            jwt_secret,
            jwt_audience,
            jwt_issuer,
            access_token_expiration,
            refresh_token_expiration,
            api_key_prefix,
        };

        // Validate configuration before returning
        config.validate()?;
        Ok(config)
    }

    /// Create default configuration - FOR DEVELOPMENT ONLY
    /// This will panic in production to prevent insecure defaults
    pub fn default() -> Self {
        Self {
            jwt_secret: "INSECURE_DEFAULT_DO_NOT_USE_IN_PRODUCTION".to_string(),
            jwt_audience: "stateset-api".to_string(),
            jwt_issuer: "stateset-auth".to_string(),
            access_token_expiration: Duration::from_secs(30 * 60), // 30 minutes
            refresh_token_expiration: Duration::from_secs(7 * 24 * 60 * 60), // 7 days
            api_key_prefix: "sk_".to_string(),
        }
    }

    /// Validate authentication configuration
    /// Returns an error if configuration is insecure or invalid
    pub fn validate(&self) -> Result<(), AuthError> {
        // Check JWT secret is not using default value
        if self.jwt_secret == "your-secret-key"
            || self.jwt_secret == "INSECURE_DEFAULT_DO_NOT_USE_IN_PRODUCTION"
            || self.jwt_secret.contains("default")
            || self.jwt_secret.contains("secret-key")
        {
            return Err(AuthError::ConfigurationError(
                "JWT secret cannot use default value. Set APP__JWT_SECRET environment variable with a secure random string.".to_string()
            ));
        }

        // Ensure JWT secret meets minimum security requirements
        const MIN_SECRET_LENGTH: usize = 32;
        if self.jwt_secret.len() < MIN_SECRET_LENGTH {
            return Err(AuthError::ConfigurationError(format!(
                "JWT secret must be at least {} characters long for security. Current length: {}",
                MIN_SECRET_LENGTH,
                self.jwt_secret.len()
            )));
        }

        // Warn if secret appears to be weak (all same character, sequential, etc.)
        if self.is_weak_secret() {
            warn!(
                "JWT secret appears weak. Use a cryptographically secure random string. \
                Generate with: openssl rand -base64 48"
            );
        }

        // Validate token expiration times are reasonable
        const MAX_ACCESS_TOKEN_DURATION: u64 = 24 * 60 * 60; // 24 hours
        const MAX_REFRESH_TOKEN_DURATION: u64 = 90 * 24 * 60 * 60; // 90 days

        if self.access_token_expiration.as_secs() > MAX_ACCESS_TOKEN_DURATION {
            return Err(AuthError::ConfigurationError(format!(
                "Access token expiration too long: {} seconds (max: {})",
                self.access_token_expiration.as_secs(),
                MAX_ACCESS_TOKEN_DURATION
            )));
        }

        if self.refresh_token_expiration.as_secs() > MAX_REFRESH_TOKEN_DURATION {
            return Err(AuthError::ConfigurationError(format!(
                "Refresh token expiration too long: {} seconds (max: {})",
                self.refresh_token_expiration.as_secs(),
                MAX_REFRESH_TOKEN_DURATION
            )));
        }

        // Access token should be shorter than refresh token
        if self.access_token_expiration >= self.refresh_token_expiration {
            return Err(AuthError::ConfigurationError(
                "Access token expiration must be shorter than refresh token expiration".to_string(),
            ));
        }

        Ok(())
    }

    /// Check if the JWT secret appears to be weak
    fn is_weak_secret(&self) -> bool {
        let secret = &self.jwt_secret;

        // Check if all characters are the same
        if let Some(first_char) = secret.chars().next() {
            if secret.chars().all(|c| c == first_char) {
                return true;
            }
        }

        // Check if it's a common pattern
        let weak_patterns = [
            "12345", "password", "test", "demo", "example", "changeme", "secret", "key", "token",
        ];

        for pattern in &weak_patterns {
            if secret.to_lowercase().contains(pattern) {
                return true;
            }
        }

        false
    }
}

/// Token blacklist backend abstraction for scalable token revocation
#[derive(Clone)]
pub enum TokenBlacklistBackend {
    /// In-memory blacklist (for local development/single instance)
    InMemory(Arc<RwLock<Vec<BlacklistedToken>>>),
    /// Redis-backed blacklist (for production/multi-instance)
    Redis {
        client: Arc<redis::Client>,
        namespace: String,
    },
}

impl std::fmt::Debug for TokenBlacklistBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TokenBlacklistBackend::InMemory(_) => write!(f, "InMemory"),
            TokenBlacklistBackend::Redis { namespace, .. } => {
                write!(f, "Redis(namespace={})", namespace)
            }
        }
    }
}

/// Authentication service that handles token issuance and validation
#[derive(Debug, Clone)]
pub struct AuthService {
    pub config: AuthConfig,
    pub db: Arc<DatabaseConnection>,
    blacklist: TokenBlacklistBackend,
}

/// Token blacklist entry (for in-memory backend)
#[derive(Clone, Debug)]
struct BlacklistedToken {
    jti: String,
    expiry: DateTime<Utc>,
}

impl AuthService {
    /// Create a new authentication service with in-memory token blacklist
    pub fn new(config: AuthConfig, db: Arc<DatabaseConnection>) -> Self {
        Self {
            config,
            db,
            blacklist: TokenBlacklistBackend::InMemory(Arc::new(RwLock::new(Vec::new()))),
        }
    }

    /// Create a new authentication service with Redis-backed token blacklist
    /// Recommended for production deployments with multiple API instances
    pub fn with_redis_blacklist(
        config: AuthConfig,
        db: Arc<DatabaseConnection>,
        redis_client: Arc<redis::Client>,
        namespace: Option<String>,
    ) -> Self {
        info!("Initializing AuthService with Redis-backed token blacklist");
        Self {
            config,
            db,
            blacklist: TokenBlacklistBackend::Redis {
                client: redis_client,
                namespace: namespace.unwrap_or_else(|| "stateset:auth:blacklist".to_string()),
            },
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

        // Get user roles and derive permissions
        let roles = match self.get_user_roles(user.id).await {
            Ok(r) => r,
            Err(AuthError::DatabaseError(msg))
                if msg.to_ascii_lowercase().contains("no such table") =>
            {
                vec!["user".to_string()]
            }
            Err(e) => return Err(e),
        };
        let permissions = self.build_permissions(user.id, &roles);

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
        let mut validation = Validation::new(Algorithm::HS256);
        validation.set_audience(&[self.config.jwt_audience.clone()]);
        validation.set_issuer(&[self.config.jwt_issuer.clone()]);

        let claims = decode::<Claims>(
            token,
            &DecodingKey::from_secret(self.config.jwt_secret.as_bytes()),
            &validation,
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

        // Calculate TTL until token expiry
        let ttl_secs = (claims.exp - Utc::now().timestamp()).max(0) as u64;

        match &self.blacklist {
            TokenBlacklistBackend::InMemory(blacklist) => {
                let expiry = Utc::now() + ChronoDuration::seconds(ttl_secs as i64);
                let blacklisted_token = BlacklistedToken {
                    jti: claims.jti.clone(),
                    expiry,
                };

                let mut blacklist = blacklist.write().await;
                blacklist.push(blacklisted_token);

                // Clean up expired tokens
                let now = Utc::now();
                blacklist.retain(|t| t.expiry > now);

                debug!(jti = %claims.jti, "Token revoked (in-memory blacklist)");
            }
            TokenBlacklistBackend::Redis { client, namespace } => {
                let key = format!("{}:{}", namespace, claims.jti);

                match client.get_async_connection().await {
                    Ok(mut conn) => {
                        // Set the key with expiry matching the token's remaining lifetime
                        // This ensures automatic cleanup when the token would have expired anyway
                        let result: Result<(), redis::RedisError> = conn
                            .set_ex(&key, "revoked", ttl_secs.max(1) as usize)
                            .await;

                        if let Err(e) = result {
                            error!(error = %e, jti = %claims.jti, "Failed to add token to Redis blacklist");
                            return Err(AuthError::InternalError(
                                "Failed to revoke token".to_string(),
                            ));
                        }

                        debug!(jti = %claims.jti, ttl_secs = ttl_secs, "Token revoked (Redis blacklist)");
                        counter!("auth.tokens_revoked_total", 1);
                    }
                    Err(e) => {
                        error!(error = %e, "Failed to connect to Redis for token revocation");
                        return Err(AuthError::InternalError(
                            "Failed to revoke token: Redis unavailable".to_string(),
                        ));
                    }
                }
            }
        }

        Ok(())
    }

    /// Check if a token is blacklisted
    async fn is_token_blacklisted(&self, token_id: &str) -> bool {
        match &self.blacklist {
            TokenBlacklistBackend::InMemory(blacklist) => {
                let blacklist = blacklist.read().await;
                blacklist.iter().any(|t| t.jti == token_id && t.expiry > Utc::now())
            }
            TokenBlacklistBackend::Redis { client, namespace } => {
                let key = format!("{}:{}", namespace, token_id);

                match client.get_async_connection().await {
                    Ok(mut conn) => {
                        let exists: Result<bool, redis::RedisError> = conn.exists(&key).await;
                        match exists {
                            Ok(true) => {
                                debug!(jti = %token_id, "Token found in Redis blacklist");
                                true
                            }
                            Ok(false) => false,
                            Err(e) => {
                                // Log error but fail closed (treat as not blacklisted)
                                // This is a security tradeoff - could also fail open
                                warn!(error = %e, jti = %token_id, "Failed to check Redis blacklist, allowing token");
                                counter!("auth.blacklist_check_failures_total", 1);
                                false
                            }
                        }
                    }
                    Err(e) => {
                        warn!(error = %e, "Failed to connect to Redis for blacklist check, allowing token");
                        counter!("auth.blacklist_check_failures_total", 1);
                        false
                    }
                }
            }
        }
    }

    /// Clean up expired tokens from the in-memory blacklist
    /// Note: Redis handles expiry automatically via TTL
    pub async fn cleanup_expired_blacklist_entries(&self) {
        if let TokenBlacklistBackend::InMemory(blacklist) = &self.blacklist {
            let mut blacklist = blacklist.write().await;
            let before_len = blacklist.len();
            let now = Utc::now();
            blacklist.retain(|t| t.expiry > now);
            let removed = before_len - blacklist.len();
            if removed > 0 {
                debug!(removed = removed, "Cleaned up expired blacklist entries");
            }
        }
    }

    /// Get a user by ID
    async fn get_user(&self, user_id: Uuid) -> Result<User, AuthError> {
        let model = UserEntity::find_by_id(user_id)
            .one(&*self.db)
            .await
            .map_err(|e| AuthError::DatabaseError(e.to_string()))?
            .ok_or(AuthError::UserNotFound)?;

        Ok(model.into())
    }

    /// Fetch explicit user roles from storage and environment overrides.
    async fn get_user_roles(&self, user_id: Uuid) -> Result<Vec<String>, AuthError> {
        let mut roles: HashSet<String> = match UserRoleEntity::find()
            .filter(user_role::Column::UserId.eq(user_id))
            .all(&*self.db)
            .await
        {
            Ok(rows) => rows.into_iter().map(|r| r.role_name).collect(),
            Err(err) => {
                let msg = err.to_string();
                if msg.to_ascii_lowercase().contains("no such table") {
                    HashSet::new()
                } else {
                    return Err(AuthError::DatabaseError(msg));
                }
            }
        };

        if let Ok(raw) = std::env::var("AUTH_DEFAULT_ROLES") {
            for role in raw.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()) {
                roles.insert(role.to_string());
            }
        }

        if Self::env_truthy("AUTH_ADMIN") {
            let allow_override =
                cfg!(test) || Self::env_truthy("STATESET_AUTH_ALLOW_ADMIN_OVERRIDE");
            if allow_override {
                roles.insert("admin".to_string());
            } else {
                warn!(
                    "AUTH_ADMIN override ignored because STATESET_AUTH_ALLOW_ADMIN_OVERRIDE is not enabled"
                );
            }
        }

        if roles.is_empty() {
            roles.insert("user".to_string());
        }

        let mut roles: Vec<String> = roles.into_iter().collect();
        roles.sort_unstable();
        Ok(roles)
    }

    /// Build the permission set for a user given their resolved roles.
    fn build_permissions(&self, _user_id: Uuid, roles: &[String]) -> Vec<String> {
        let mut permissions: HashSet<String> = HashSet::new();

        for role in roles {
            if let Some(role_def) = rbac::ROLES.get(&role.to_lowercase()) {
                permissions.extend(role_def.permissions.iter().cloned());
            }
        }

        if let Ok(raw) = std::env::var("AUTH_DEFAULT_PERMISSIONS") {
            for permission in raw.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()) {
                permissions.insert(permission.to_string());
            }
        }

        let mut permissions: Vec<String> = permissions.into_iter().collect();
        permissions.sort_unstable();
        permissions
    }

    fn env_truthy(var: &str) -> bool {
        std::env::var(var)
            .map(|v| matches!(v.trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes"))
            .unwrap_or(false)
    }

    /// Store a refresh token
    async fn store_refresh_token(
        &self,
        user_id: Uuid,
        token_id: &str,
        expiry: DateTime<Utc>,
    ) -> Result<(), AuthError> {
        let user_exists = UserEntity::find_by_id(user_id)
            .one(&*self.db)
            .await
            .map_err(|e| AuthError::DatabaseError(e.to_string()))?
            .is_some();

        if !user_exists {
            debug!(
                "Skipping refresh token persistence for unknown user {}",
                user_id
            );
            return Ok(());
        }

        let model = refresh_token::ActiveModel {
            id: Set(Uuid::new_v4()),
            user_id: Set(user_id),
            token_id: Set(token_id.to_string()),
            created_at: Set(Utc::now()),
            expires_at: Set(expiry),
            revoked: Set(false),
        };

        model
            .insert(&*self.db)
            .await
            .map_err(|e| AuthError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    /// Verify a refresh token
    async fn verify_refresh_token(&self, user_id: Uuid, token_id: &str) -> Result<bool, AuthError> {
        let record = RefreshTokenEntity::find()
            .filter(refresh_token::Column::UserId.eq(user_id))
            .filter(refresh_token::Column::TokenId.eq(token_id))
            .one(&*self.db)
            .await
            .map_err(|e| AuthError::DatabaseError(e.to_string()))?;

        match record {
            Some(token) if !token.revoked && token.expires_at > Utc::now() => Ok(true),
            _ => Ok(false),
        }
    }

    /// Revoke a refresh token
    async fn revoke_refresh_token(&self, user_id: Uuid, token_id: &str) -> Result<(), AuthError> {
        if let Some(record) = RefreshTokenEntity::find()
            .filter(refresh_token::Column::UserId.eq(user_id))
            .filter(refresh_token::Column::TokenId.eq(token_id))
            .one(&*self.db)
            .await
            .map_err(|e| AuthError::DatabaseError(e.to_string()))?
        {
            let mut active: refresh_token::ActiveModel = record.into();
            active.revoked = Set(true);
            active
                .update(&*self.db)
                .await
                .map_err(|e| AuthError::DatabaseError(e.to_string()))?;
        }

        Ok(())
    }

    /// Validate API key
    pub async fn validate_api_key(&self, api_key: &str) -> Result<ApiKey, AuthError> {
        if !api_key.starts_with(&self.config.api_key_prefix) {
            return Err(AuthError::InvalidApiKey);
        }

        let hash = Self::hash_api_key(api_key);

        let record = ApiKeyEntity::find()
            .filter(api_key::Column::KeyHash.eq(hash))
            .filter(api_key::Column::Revoked.eq(false))
            .one(&*self.db)
            .await
            .map_err(|e| AuthError::DatabaseError(e.to_string()))?
            .ok_or(AuthError::InvalidApiKey)?;

        if let Some(expires_at) = record.expires_at {
            if expires_at < Utc::now() {
                return Err(AuthError::ExpiredApiKey);
            }
        }

        let mut roles = self.get_user_roles(record.user_id).await?;
        if roles.is_empty() {
            roles.push("api".to_string());
        }

        let mut permissions: HashSet<String> = self
            .build_permissions(record.user_id, &roles)
            .into_iter()
            .collect();

        let key_specific_permissions = ApiKeyPermissionEntity::find()
            .filter(api_key_permission::Column::ApiKeyId.eq(record.id))
            .all(&*self.db)
            .await
            .map_err(|e| AuthError::DatabaseError(e.to_string()))?;

        for perm in key_specific_permissions {
            permissions.insert(perm.permission);
        }

        let mut permissions: Vec<String> = permissions.into_iter().collect();
        permissions.sort_unstable();

        let now = Utc::now();
        let mut active: api_key::ActiveModel = record.clone().into();
        active.last_used_at = Set(Some(now));
        active
            .update(&*self.db)
            .await
            .map_err(|e| AuthError::DatabaseError(e.to_string()))?;

        Ok(ApiKey {
            id: record.id,
            name: record.name,
            key: api_key.to_string(),
            user_id: record.user_id,
            roles,
            permissions,
            tenant_id: record.tenant_id,
            created_at: record.created_at,
            expires_at: record.expires_at,
            last_used_at: Some(now),
        })
    }

    /// Authenticate a user by credentials.
    pub async fn authenticate_user(&self, email: &str, password: &str) -> Result<User, AuthError> {
        let credentials = email.trim();
        if credentials.is_empty() || password.is_empty() {
            return Err(AuthError::InvalidCredentials);
        }

        let model = self
            .find_user_by_email(credentials)
            .await?
            .ok_or(AuthError::InvalidCredentials)?;

        if !model.active {
            return Err(AuthError::InvalidCredentials);
        }

        let verified = self.verify_password(&model.password_hash, password)?;
        if !verified {
            return Err(AuthError::InvalidCredentials);
        }

        Ok(model.into())
    }

    async fn find_user_by_email(&self, email: &str) -> Result<Option<user::Model>, AuthError> {
        UserEntity::find()
            .filter(user::Column::Email.eq(email))
            .one(&*self.db)
            .await
            .map_err(|e| AuthError::DatabaseError(e.to_string()))
    }

    fn verify_password(&self, stored_hash: &str, candidate: &str) -> Result<bool, AuthError> {
        if stored_hash.trim().is_empty() {
            return Ok(false);
        }

        if stored_hash.starts_with("hashed_") {
            // Legacy/dev-only hashing helper used in a few tests.
            let expected = format!("hashed_{}", candidate);
            let matches = stored_hash == expected;
            if matches {
                warn!("Using legacy test password hash; please migrate to Argon2");
            }
            return Ok(matches);
        }

        let parsed = PasswordHash::new(stored_hash)
            .map_err(|e| AuthError::InternalError(format!("invalid password hash: {}", e)))?;

        match Argon2::default().verify_password(candidate.as_bytes(), &parsed) {
            Ok(_) => Ok(true),
            Err(argon2::password_hash::Error::Password) => Ok(false),
            Err(e) => Err(AuthError::InternalError(format!(
                "password verification failed: {}",
                e
            ))),
        }
    }

    fn hash_api_key(value: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(value.as_bytes());
        hex::encode(hasher.finalize())
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

impl From<user::Model> for User {
    fn from(model: user::Model) -> Self {
        Self {
            id: model.id,
            name: model.name,
            email: model.email,
            password_hash: model.password_hash,
            tenant_id: model.tenant_id,
            active: model.active,
            created_at: model.created_at,
            updated_at: model.updated_at,
        }
    }
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

    #[error("Configuration error: {0}")]
    ConfigurationError(String),
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
            Self::ConfigurationError(msg) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "AUTH_CONFIGURATION_ERROR",
                format!("Invalid configuration: {}", msg),
            ),
        };

        // Emit auth failure metric
        counter!(
            "auth_failures_total",
            1,
            "code" => error_code.to_string(),
            "status" => status.as_u16().to_string(),
        );
        // Also update custom metrics registry for visibility in /metrics
        let _ = {
            #[allow(unused_imports)]
            use crate::metrics::increment_counter;
            increment_counter("auth_failures_total");
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

/// Extract AuthUser from request extensions.
/// Assumes `auth_middleware` has populated `AuthUser` in request extensions.
#[async_trait::async_trait]
#[async_trait::async_trait]
impl<S> FromRequestParts<S> for AuthUser
where
    S: Send + Sync,
{
    type Rejection = AuthError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<AuthUser>()
            .cloned()
            .ok_or(AuthError::MissingAuth)
    }
}

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
    request: Request,
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
    request: Request,
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
        .route("/logout", axum::routing::post(logout_handler))
        .route("/api-keys", axum::routing::post(create_api_key_handler))
        .layer(DefaultBodyLimit::max(1024 * 64)) // 64KB limit
}

/// Login handler
pub async fn login_handler(
    State(auth_service): State<Arc<AuthService>>,
    Json(credentials): Json<LoginCredentials>,
) -> Result<Json<TokenPair>, AuthError> {
    let user = auth_service
        .authenticate_user(&credentials.email, &credentials.password)
        .await?;

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
                return Ok(Json(
                    serde_json::json!({ "message": "Successfully logged out" }),
                ));
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
