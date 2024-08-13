use actix_web::{dev::ServiceRequest, Error, HttpMessage};
use actix_web_httpauth::extractors::bearer::BearerAuth;
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use std::future::{ready, Ready};

#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    sub: String,
    exp: usize,
    role: String,
}

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
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;

    actix_web::dev::forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let bearer_token = req.headers().get("Authorization")
            .and_then(|h| h.to_str().ok())
            .and_then(|h| h.strip_prefix("Bearer "))
            .map(|s| s.to_string());

        if let Some(token) = bearer_token {
            let secret = req.app_data::<web::Data<AppConfig>>().unwrap().jwt_secret.as_bytes();
            match decode::<Claims>(&token, &DecodingKey::from_secret(secret), &Validation::default()) {
                Ok(token_data) => {
                    if self.allowed_roles.contains(&token_data.claims.role) {
                        let fut = self.service.call(req);
                        return Box::pin(async move {
                            let res = fut.await?;
                            Ok(res)
                        });
                    }
                }
                Err(_) => {}
            }
        }

        Box::pin(async move {
            Err(actix_web::error::ErrorUnauthorized("Invalid token or insufficient permissions"))
        })
    }
}