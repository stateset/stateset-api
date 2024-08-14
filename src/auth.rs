use actix_web::{dev::ServiceRequest, Error, HttpMessage};
use actix_web_httpauth::extractors::bearer::BearerAuth;
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use std::future::{ready, Ready};
use std::pin::Pin;
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
}

/// Authentication middleware
pub struct AuthMiddleware {
    allowed_roles: Vec<String>,
}

impl AuthMiddleware {
    pub fn new(allowed_roles: Vec<&str>) -> Self {
        AuthMiddleware {
            allowed_roles: allowed_roles.into_iter().map(String::from).collect(),
        }
    }
}

impl<S, B> actix_web::dev::Transform<S, ServiceRequest> for AuthMiddleware
where
    S: actix_web::dev::Service<ServiceRequest, Response = actix_web::dev::ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = actix_web::dev::ServiceResponse<B>;
    type Error = Error;
    type InitError = ();
    type Transform = AuthMiddlewareService<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(AuthMiddlewareService { service, allowed_roles: self.allowed_roles.clone() }))
    }
}

pub struct AuthMiddlewareService<S> {
    service: S,
    allowed_roles: Vec<String>,
}

impl<S, B> actix_web::dev::Service<ServiceRequest> for AuthMiddlewareService<S>
where
    S: actix_web::dev::Service<ServiceRequest, Response = actix_web::dev::ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = actix_web::dev::ServiceResponse<B>;
    type Error = Error;
    type Future = Pin<Box<dyn std::future::Future<Output = Result<Self::Response, Self::Error>>>>;

    actix_web::dev::forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let bearer_token = req.headers().get("Authorization")
            .and_then(|h| h.to_str().ok())
            .and_then(|h| h.strip_prefix("Bearer "))
            .map(|s| s.to_string());

        let allowed_roles = self.allowed_roles.clone();
        let fut = self.service.call(req);

        Box::pin(async move {
            if let Some(token) = bearer_token {
                match validate_token(&token, &allowed_roles) {
                    Ok(claims) => {
                        fut.await
                    }
                    Err(e) => Err(actix_web::error::ErrorUnauthorized(e))
                }
            } else {
                Err(actix_web::error::ErrorUnauthorized("No authorization token provided"))
            }
        })
    }
}

/// Generates a JWT token
pub fn generate_token(user_id: &str, role: &str, secret: &[u8], expiration: i64) -> Result<String, AuthError> {
    let exp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_secs() as usize + expiration as usize;

    let claims = Claims {
        sub: user_id.to_owned(),
        exp,
        role: role.to_owned(),
    };

    encode(&Header::default(), &claims, &EncodingKey::from_secret(secret))
        .map_err(AuthError::JWTError)
}

/// Validates a JWT token
pub fn validate_token(token: &str, allowed_roles: &[String]) -> Result<Claims, AuthError> {
    let secret = std::env::var("JWT_SECRET").expect("JWT_SECRET must be set");
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_generate_and_validate_token() {
        std::env::set_var("JWT_SECRET", "test_secret");
        let user_id = "123";
        let role = "user";
        let secret = "test_secret".as_bytes();
        let expiration = 3600; // 1 hour

        let token = generate_token(user_id, role, secret, expiration).unwrap();
        let claims = validate_token(&token, &["user".to_string()]).unwrap();

        assert_eq!(claims.sub, user_id);
        assert_eq!(claims.role, role);
    }

    #[test]
    fn test_expired_token() {
        std::env::set_var("JWT_SECRET", "test_secret");
        let user_id = "123";
        let role = "user";
        let secret = "test_secret".as_bytes();
        let expiration = -1; // Expired token

        let token = generate_token(user_id, role, secret, expiration).unwrap();
        let result = validate_token(&token, &["user".to_string()]);

        assert!(matches!(result, Err(AuthError::ExpiredToken)));
    }

    #[test]
    fn test_insufficient_permissions() {
        std::env::set_var("JWT_SECRET", "test_secret");
        let user_id = "123";
        let role = "user";
        let secret = "test_secret".as_bytes();
        let expiration = 3600;

        let token = generate_token(user_id, role, secret, expiration).unwrap();
        let result = validate_token(&token, &["admin".to_string()]);

        assert!(matches!(result, Err(AuthError::InsufficientPermissions)));
    }
}