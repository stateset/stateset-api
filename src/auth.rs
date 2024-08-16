use axum::{
    extract::State,
    http::{Request, StatusCode},
    middleware::{FromRequestParts, Next},
    response::{IntoResponse, Response},
    RequestPartsExt,
};
use http::request::Parts;
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;

/// Claims structure for JWT
#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub exp: usize,
    pub role: String,
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
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        let status = match self {
            AuthError::InvalidToken => StatusCode::UNAUTHORIZED,
            AuthError::ExpiredToken => StatusCode::UNAUTHORIZED,
            AuthError::InsufficientPermissions => StatusCode::FORBIDDEN,
            AuthError::JWTError(_) => StatusCode::UNAUTHORIZED,
            AuthError::MissingAuthHeader => StatusCode::UNAUTHORIZED,
        };

        (status, self.to_string()).into_response()
    }
}

#[derive(Clone)]
pub struct AuthConfig {
    pub secret: String,
    pub allowed_roles: Vec<String>,
}

pub async fn auth_middleware<B>(
    State(config): State<Arc<AuthConfig>>,
    mut req: Request<B>,
    next: Next<B>,
) -> Result<Response, AuthError> {
    let bearer_token = req
        .headers()
        .get("Authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "))
        .ok_or(AuthError::MissingAuthHeader)?;

    let claims = validate_token(bearer_token, &config.secret, &config.allowed_roles)?;
    req.extensions_mut().insert(claims);

    Ok(next.run(req).await)
}

/// Generates a JWT token
pub fn generate_token(user_id: &str, role: &str, secret: &str, expiration: i64) -> Result<String, AuthError> {
    let exp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_secs() as usize + expiration as usize;

    let claims = Claims {
        sub: user_id.to_owned(),
        exp,
        role: role.to_owned(),
    };

    encode(&Header::default(), &claims, &EncodingKey::from_secret(secret.as_bytes()))
        .map_err(AuthError::JWTError)
}

/// Validates a JWT token
pub fn validate_token(token: &str, secret: &str, allowed_roles: &[String]) -> Result<Claims, AuthError> {
    let key = DecodingKey::from_secret(secret.as_bytes());
    let validation = Validation::default();

    let token_data = decode::<Claims>(token, &key, &validation)
        .map_err(|e| match e.kind() {
            jsonwebtoken::errors::ErrorKind::ExpiredSignature => AuthError::ExpiredToken,
            _ => AuthError::InvalidToken,
        })?;

    if !allowed_roles.contains(&token_data.claims.role) {
        return Err(AuthError::InsufficientPermissions);
    }

    Ok(token_data.claims)
}

// Extractor for authenticated user
pub struct AuthUser(pub Claims);

#[axum::async_trait]
impl<S> FromRequestParts<S> for AuthUser
where
    S: Send + Sync,
{
    type Rejection = AuthError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
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
    use tower::ServiceExt;

    async fn protected_route(AuthUser(claims): AuthUser) -> String {
        format!("Hello, {}!", claims.sub)
    }

    #[tokio::test]
    async fn test_auth_middleware() {
        let secret = "test_secret".to_string();
        let config = Arc::new(AuthConfig {
            secret: secret.clone(),
            allowed_roles: vec!["user".to_string()],
        });

        let app = Router::new()
            .route("/protected", get(protected_route))
            .layer(axum::middleware::from_fn_with_state(config.clone(), auth_middleware))
            .with_state(config);

        // Test valid token
        let token = generate_token("123", "user", &secret, 3600).unwrap();
        let request = Request::builder()
            .uri("/protected")
            .header("Authorization", format!("Bearer {}", token))
            .body(Body::empty())
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        // Test invalid token
        let request = Request::builder()
            .uri("/protected")
            .header("Authorization", "Bearer invalid_token")
            .body(Body::empty())
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        // Test missing token
        let request = Request::builder()
            .uri("/protected")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn test_generate_and_validate_token() {
        let user_id = "123";
        let role = "user";
        let secret = "test_secret";
        let expiration = 3600; // 1 hour

        let token = generate_token(user_id, role, secret, expiration).unwrap();
        let claims = validate_token(&token, secret, &["user".to_string()]).unwrap();

        assert_eq!(claims.sub, user_id);
        assert_eq!(claims.role, role);
    }

    #[test]
    fn test_expired_token() {
        let user_id = "123";
        let role = "user";
        let secret = "test_secret";
        let expiration = -1; // Expired token

        let token = generate_token(user_id, role, secret, expiration).unwrap();
        let result = validate_token(&token, secret, &["user".to_string()]);

        assert!(matches!(result, Err(AuthError::ExpiredToken)));
    }

    #[test]
    fn test_insufficient_permissions() {
        let user_id = "123";
        let role = "user";
        let secret = "test_secret";
        let expiration = 3600;

        let token = generate_token(user_id, role, secret, expiration).unwrap();
        let result = validate_token(&token, secret, &["admin".to_string()]);

        assert!(matches!(result, Err(AuthError::InsufficientPermissions)));
    }
}