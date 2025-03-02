use axum::{
    async_trait,
    extract::{FromRequestParts, State},
    http::{Request, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    Json,
};
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use thiserror::Error;
use tracing::{error, info, instrument};
use validator::Validate;

/// JWT Claims structure with validation
#[derive(Debug, Serialize, Deserialize, Clone, Validate)]
pub struct Claims {
    #[validate(length(min = 1))]
    pub sub: String,
    pub exp: usize,
    #[validate(length(min = 1))]
    pub iss: String,
    #[validate(length(min = 1))]
    pub aud: String,
    #[validate(length(min = 1))]
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub permissions: Option<Vec<String>>,
}

impl Claims {
    /// Creates new claims with given parameters
    pub fn new(
        sub: String,
        role: String,
        permissions: Option<Vec<String>>,
        issuer: String,
        audience: String,
        expiration: Duration,
    ) -> Result<Self, validator::ValidationErrors> {
        let exp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_secs() as usize + expiration.as_secs() as usize;

        let claims = Self {
            sub,
            exp,
            iss: issuer,
            aud: audience,
            role,
            permissions,
        };
        claims.validate()?;
        Ok(claims)
    }
}

/// Authentication error types
#[derive(Debug, Error)]
pub enum AuthError {
    #[error("Invalid token: {0}")]
    InvalidToken(String),
    #[error("Token expired")]
    ExpiredToken,
    #[error("Insufficient permissions: required {0}")]
    InsufficientPermissions(String),
    #[error("JWT processing error: {0}")]
    JWTError(#[from] jsonwebtoken::errors::Error),
    #[error("Missing Authorization header")]
    MissingAuthHeader,
    #[error("Invalid issuer or audience")]
    InvalidIssuerAudience,
    #[error("Validation error: {0}")]
    Validation(#[from] validator::ValidationErrors),
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            Self::InvalidToken(msg) => (StatusCode::UNAUTHORIZED, msg.as_str()),
            Self::ExpiredToken => (StatusCode::UNAUTHORIZED, "Token has expired"),
            Self::InsufficientPermissions(msg) => (StatusCode::FORBIDDEN, msg.as_str()),
            Self::JWTError(_) => (StatusCode::UNAUTHORIZED, "Token processing failed"),
            Self::MissingAuthHeader => (StatusCode::UNAUTHORIZED, "Missing Authorization header"),
            Self::InvalidIssuerAudience => (StatusCode::UNAUTHORIZED, "Invalid issuer or audience"),
            Self::Validation(_) => (StatusCode::BAD_REQUEST, "Invalid claims data"),
        };

        let body = serde_json::json!({
            "error": message,
            "timestamp": chrono::Utc::now().to_rfc3339(),
        });

        (status, Json(body)).into_response()
    }
}

/// Authentication configuration
#[derive(Clone, Validate)]
pub struct AuthConfig {
    #[validate(length(min = 32))]
    pub secret: String,
    #[validate(length(min = 1))]
    pub issuer: String,
    #[validate(length(min = 1))]
    pub audience: String,
    #[validate]
    pub allowed_roles: HashSet<String>,
    #[validate(range(min = 60))]
    pub token_expiration: usize,
}

impl AuthConfig {
    pub fn new(
        secret: String,
        issuer: String,
        audience: String,
        allowed_roles: HashSet<String>,
        token_expiration: usize,
    ) -> Result<Self, validator::ValidationErrors> {
        let config = Self {
            secret,
            issuer,
            audience,
            allowed_roles,
            token_expiration,
        };
        config.validate()?;
        Ok(config)
    }
}

/// Application state with auth configuration
#[derive(Clone)]
pub struct AppState {
    pub auth_config: Arc<AuthConfig>,
}

/// Authentication middleware
#[instrument(skip_all, fields(method = %req.method(), uri = %req.uri()))]
pub async fn auth_middleware<B>(
    State(state): State<AppState>,
    mut req: Request<B>,
    next: Next<B>,
) -> Result<Response, AuthError> {
    let bearer_token = req.headers()
        .get("Authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "))
        .ok_or(AuthError::MissingAuthHeader)?;

    let claims = validate_token(bearer_token, &state.auth_config)?;
    info!(user_id = %claims.sub, role = %claims.role, "User authenticated");

    req.extensions_mut().insert(claims);
    Ok(next.run(req).await)
}

/// Generates a new JWT token
pub fn generate_token(
    user_id: &str,
    role: &str,
    permissions: Option<Vec<String>>,
    config: &AuthConfig,
) -> Result<String, AuthError> {
    let claims = Claims::new(
        user_id.to_string(),
        role.to_string(),
        permissions,
        config.issuer.clone(),
        config.audience.clone(),
        Duration::from_secs(config.token_expiration as u64),
    )?;

    encode(
        &Header::new(Algorithm::HS256),
        &claims,
        &EncodingKey::from_secret(config.secret.as_bytes()),
    )
    .map_err(AuthError::JWTError)
}

/// Validates a JWT token
pub fn validate_token(token: &str, config: &AuthConfig) -> Result<Claims, AuthError> {
    let mut validation = Validation::new(Algorithm::HS256);
    validation.set_audience(&[&config.audience]);
    validation.set_issuer(&[&config.issuer]);
    validation.validate_exp = true;

    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(config.secret.as_bytes()),
        &validation,
    ).map_err(|e| match e.kind() {
        jsonwebtoken::errors::ErrorKind::ExpiredSignature => AuthError::ExpiredToken,
        jsonwebtoken::errors::ErrorKind::InvalidIssuer | 
        jsonwebtoken::errors::ErrorKind::InvalidAudience => AuthError::InvalidIssuerAudience,
        _ => AuthError::InvalidToken(format!("JWT validation failed: {}", e)),
    })?;

    if !config.allowed_roles.contains(&token_data.claims.role) {
        return Err(AuthError::InsufficientPermissions(
            format!("Role '{}' not allowed", token_data.claims.role)
        ));
    }

    Ok(token_data.claims)
}

/// Authenticated user extractor
#[derive(Debug)]
pub struct AuthUser(pub Claims);

#[async_trait]
impl<S> FromRequestParts<S> for AuthUser
where
    S: Send + Sync,
{
    type Rejection = AuthError;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        _state: &S
    ) -> Result<Self, Self::Rejection> {
        parts.extensions
            .get::<Claims>()
            .cloned()
            .map(AuthUser)
            .ok_or_else(|| AuthError::InvalidToken("No claims found in request".to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{routing::get, Router};
    use http::{Request, StatusCode};
    use tower::ServiceExt;
    use serde_json::json;

    async fn protected_handler(AuthUser(claims): AuthUser) -> impl IntoResponse {
        Json(json!({
            "message": format!("Hello, {}!", claims.sub),
            "role": claims.role,
            "permissions": claims.permissions.unwrap_or_default(),
        }))
    }

    fn setup_test_config() -> AuthConfig {
        AuthConfig::new(
            "supersecretkeythatislongenough12345".to_string(),
            "test_issuer".to_string(),
            "test_audience".to_string(),
            ["user".to_string(), "admin".to_string()].into_iter().collect(),
            3600,
        ).unwrap()
    }

    #[tokio::test]
    async fn test_auth_flow() {
        let config = setup_test_config();
        let state = AppState { auth_config: Arc::new(config.clone()) };
        
        let app = Router::new()
            .route("/protected", get(protected_handler))
            .layer(middleware::from_fn_with_state(state.clone(), auth_middleware))
            .with_state(state);

        let token = generate_token("test_user", "user", Some(vec!["read".to_string()]), &config).unwrap();
        
        let request = Request::builder()
            .uri("/protected")
            .header("Authorization", format!("Bearer {}", token))
            .body(hyper::Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(body["message"], "Hello, test_user!");
    }

    #[tokio::test]
    async fn test_invalid_token() {
        let config = setup_test_config();
        let state = AppState { auth_config: Arc::new(config) };
        
        let app = Router::new()
            .route("/protected", get(protected_handler))
            .layer(middleware::from_fn_with_state(state.clone(), auth_middleware))
            .with_state(state);

        let request = Request::builder()
            .uri("/protected")
            .header("Authorization", "Bearer invalidtoken")
            .body(hyper::Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
}