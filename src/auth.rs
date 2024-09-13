use axum::{
    async_trait,
    extract::{FromRequestParts, State},
    http::{Request, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
};
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};
use thiserror::Error;
use tracing::{error, info, instrument};

/// Claims structure for JWT
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String,             // Subject (e.g., user ID)
    pub exp: usize,              // Expiration time (as UTC timestamp)
    pub iss: String,             // Issuer
    pub aud: String,             // Audience
    pub role: String,            // User role
    pub permissions: Option<Vec<String>>, // Optional permissions
}

/// Custom error type for authentication errors
#[derive(Debug, Error)]
pub enum AuthError {
    #[error("Invalid token")]
    InvalidToken,
    #[error("Expired token")]
    ExpiredToken,
    #[error("Insufficient permissions")]
    InsufficientPermissions,
    #[error("JWT error: {0}")]
    JWTError(#[from] jsonwebtoken::errors::Error),
    #[error("Missing authorization header")]
    MissingAuthHeader,
    #[error("Invalid issuer or audience")]
    InvalidIssuerAudience,
}

/// Implement IntoResponse to convert AuthError into HTTP responses
impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            AuthError::InvalidToken => (StatusCode::UNAUTHORIZED, "Invalid token"),
            AuthError::ExpiredToken => (StatusCode::UNAUTHORIZED, "Expired token"),
            AuthError::InsufficientPermissions => (StatusCode::FORBIDDEN, "Insufficient permissions"),
            AuthError::JWTError(_) => (StatusCode::UNAUTHORIZED, "Invalid token"),
            AuthError::MissingAuthHeader => (StatusCode::UNAUTHORIZED, "Missing authorization header"),
            AuthError::InvalidIssuerAudience => (StatusCode::UNAUTHORIZED, "Invalid issuer or audience"),
        };

        let body = serde_json::json!({
            "error": message,
        });

        (status, axum::Json(body)).into_response()
    }
}

/// Configuration for authentication
#[derive(Clone)]
pub struct AuthConfig {
    pub secret: String,                // Secret key for signing tokens
    pub issuer: String,                // Expected issuer
    pub audience: String,              // Expected audience
    pub allowed_roles: HashSet<String>, // Set of allowed roles
    pub token_expiration: usize,       // Token expiration in seconds
}

/// Service state containing the authentication configuration
#[derive(Clone)]
pub struct AppState {
    pub auth_config: Arc<AuthConfig>,
}

/// Middleware for authenticating requests
#[instrument(skip_all, fields(method = %req.method(), uri = %req.uri().path()))]
pub async fn auth_middleware<B>(
    State(state): State<AppState>,
    mut req: Request<B>,
    next: Next<B>,
) -> Result<Response, AuthError> {
    // Extract the Authorization header
    let bearer_token = req
        .headers()
        .get("Authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "))
        .ok_or(AuthError::MissingAuthHeader)?;

    // Validate the token
    let claims = validate_token(bearer_token, &state.auth_config)?;
    info!(
        "Authenticated user {} with role {}",
        claims.sub, claims.role
    );

    // Insert the claims into request extensions for later use
    req.extensions_mut().insert(claims);

    // Proceed to the next handler
    Ok(next.run(req).await)
}

/// Generates a JWT token
pub fn generate_token(
    user_id: &str,
    role: &str,
    permissions: Option<Vec<String>>,
    config: &AuthConfig,
) -> Result<String, AuthError> {
    let expiration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_secs() as usize
        + config.token_expiration;

    let claims = Claims {
        sub: user_id.to_owned(),
        exp: expiration,
        iss: config.issuer.clone(),
        aud: config.audience.clone(),
        role: role.to_owned(),
        permissions,
    };

    let header = Header::new(Algorithm::HS256);
    encode(
        &header,
        &claims,
        &EncodingKey::from_secret(config.secret.as_bytes()),
    )
    .map_err(AuthError::JWTError)
}

/// Validates a JWT token
pub fn validate_token(token: &str, config: &AuthConfig) -> Result<Claims, AuthError> {
    let decoding_key = DecodingKey::from_secret(config.secret.as_bytes());

    let mut validation = Validation::new(Algorithm::HS256);
    validation.set_audience(&[config.audience.clone()]);
    validation.set_issuer(&[config.issuer.clone()]);
    validation.validate_exp = true;

    let token_data = decode::<Claims>(token, &decoding_key, &validation)
        .map_err(|e| match e.kind() {
            jsonwebtoken::errors::ErrorKind::ExpiredSignature => AuthError::ExpiredToken,
            jsonwebtoken::errors::ErrorKind::InvalidIssuer => AuthError::InvalidIssuerAudience,
            jsonwebtoken::errors::ErrorKind::InvalidAudience => AuthError::InvalidIssuerAudience,
            _ => AuthError::InvalidToken,
        })?;

    // Check if the user's role is allowed
    if !config.allowed_roles.contains(&token_data.claims.role) {
        return Err(AuthError::InsufficientPermissions);
    }

    Ok(token_data.claims)
}

/// Extractor for authenticated user
pub struct AuthUser(pub Claims);

#[async_trait]
impl<S> FromRequestParts<S> for AuthUser
where
    S: Send + Sync,
{
    type Rejection = AuthError;

    #[instrument(skip_all)]
    async fn from_request_parts(parts: &mut axum::http::request::Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<Claims>()
            .cloned()
            .map(AuthUser)
            .ok_or(AuthError::InvalidToken)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        routing::get,
        Router,
    };
    use http::{Request, StatusCode};
    use serde_json::json;
    use std::collections::HashSet;
    use tower::ServiceExt; // for `oneshot` and `ready`

    /// Handler for protected routes
    async fn protected_route(AuthUser(claims): AuthUser) -> impl IntoResponse {
        json!({
            "message": format!("Hello, {}!", claims.sub),
            "role": claims.role,
            "permissions": claims.permissions.unwrap_or_default(),
        })
    }

    #[tokio::test]
    async fn test_auth_middleware() {
        // Initialize tracing subscriber for logging in tests
        let _ = tracing_subscriber::fmt::try_init();

        // Define auth configuration
        let auth_config = AuthConfig {
            secret: "test_secret".to_string(),
            issuer: "test_issuer".to_string(),
            audience: "test_audience".to_string(),
            allowed_roles: ["user".to_string(), "admin".to_string()].iter().cloned().collect(),
            token_expiration: 3600, // 1 hour
        };

        let app_state = AppState {
            auth_config: Arc::new(auth_config.clone()),
        };

        // Build the application with the authentication middleware
        let app = Router::new()
            .route("/protected", get(protected_route))
            .layer(middleware::from_fn_with_state(
                app_state.clone(),
                auth_middleware,
            ))
            .with_state(app_state.clone());

        // Generate a valid token
        let token = generate_token("user123", "user", Some(vec!["read".to_string()]), &auth_config).unwrap();

        // Create a request with a valid token
        let valid_request = Request::builder()
            .uri("/protected")
            .header("Authorization", format!("Bearer {}", token))
            .body(Body::empty())
            .unwrap();

        // Send the request and assert the response
        let response = app.clone().oneshot(valid_request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body_bytes = hyper::body::to_bytes(response.into_body()).await.unwrap();
        let body: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
        assert_eq!(body["message"], "Hello, user123!");
        assert_eq!(body["role"], "user");
        assert_eq!(body["permissions"][0], "read");

        // Create a request with an invalid token
        let invalid_request = Request::builder()
            .uri("/protected")
            .header("Authorization", "Bearer invalid_token")
            .body(Body::empty())
            .unwrap();

        let response = app.clone().oneshot(invalid_request).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        // Create a request with a missing token
        let missing_token_request = Request::builder()
            .uri("/protected")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(missing_token_request).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn test_generate_and_validate_token() {
        let auth_config = AuthConfig {
            secret: "test_secret".to_string(),
            issuer: "test_issuer".to_string(),
            audience: "test_audience".to_string(),
            allowed_roles: ["user".to_string()].iter().cloned().collect(),
            token_expiration: 3600,
        };

        // Generate a token
        let token = generate_token("user123", "user", Some(vec!["read".to_string()]), &auth_config).unwrap();

        // Validate the token
        let claims = validate_token(&token, &auth_config).unwrap();
        assert_eq!(claims.sub, "user123");
        assert_eq!(claims.role, "user");
        assert_eq!(claims.permissions.unwrap()[0], "read");
    }

    #[test]
    fn test_expired_token() {
        let auth_config = AuthConfig {
            secret: "test_secret".to_string(),
            issuer: "test_issuer".to_string(),
            audience: "test_audience".to_string(),
            allowed_roles: ["user".to_string()].iter().cloned().collect(),
            token_expiration: 0, // Immediate expiration
        };

        // Generate a token that is already expired
        let token = generate_token("user123", "user", Some(vec!["read".to_string()]), &auth_config).unwrap();

        // Validate the token
        let result = validate_token(&token, &auth_config);
        assert!(matches!(result, Err(AuthError::ExpiredToken)));
    }

    #[test]
    fn test_insufficient_permissions() {
        let auth_config = AuthConfig {
            secret: "test_secret".to_string(),
            issuer: "test_issuer".to_string(),
            audience: "test_audience".to_string(),
            allowed_roles: ["admin".to_string()].iter().cloned().collect(), // Only admin allowed
            token_expiration: 3600,
        };

        // Generate a token with role 'user' which is not allowed
        let token = generate_token("user123", "user", Some(vec!["read".to_string()]), &auth_config).unwrap();

        // Validate the token
        let result = validate_token(&token, &auth_config);
        assert!(matches!(result, Err(AuthError::InsufficientPermissions)));
    }

    #[test]
    fn test_invalid_issuer_audience() {
        let auth_config = AuthConfig {
            secret: "test_secret".to_string(),
            issuer: "expected_issuer".to_string(),
            audience: "expected_audience".to_string(),
            allowed_roles: ["user".to_string()].iter().cloned().collect(),
            token_expiration: 3600,
        };

        // Generate a token with incorrect issuer and audience
        let claims = Claims {
            sub: "user123".to_string(),
            exp: (SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("Time went backwards")
                .as_secs() as usize)
                + 3600,
            iss: "wrong_issuer".to_string(),
            aud: "wrong_audience".to_string(),
            role: "user".to_string(),
            permissions: Some(vec!["read".to_string()]),
        };

        let header = Header::new(Algorithm::HS256);
        let token = encode(
            &header,
            &claims,
            &EncodingKey::from_secret(auth_config.secret.as_bytes()),
        )
        .unwrap();

        // Validate the token
        let result = validate_token(&token, &auth_config);
        assert!(matches!(result, Err(AuthError::InvalidIssuerAudience)));
    }
}
