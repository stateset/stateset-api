mod common;

use axum::http::{Method, StatusCode};
use chrono::Utc;
use stateset_api::auth::{Claims, User};
use uuid::Uuid;

use common::TestApp;

#[tokio::test]
#[ignore = "requires SQLite and Redis integration environment"]
async fn test_protected_endpoint_requires_auth() {
    let app = TestApp::new().await;

    let response = app.request(Method::GET, "/api/v1/orders", None, None).await;

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
#[ignore = "requires SQLite and Redis integration environment"]
async fn test_valid_token_allows_access() {
    let app = TestApp::new().await;

    let response = app
        .request_authenticated(Method::GET, "/api/v1/orders", None)
        .await;

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
#[ignore = "requires SQLite and Redis integration environment"]
async fn test_invalid_token_rejected() {
    let app = TestApp::new().await;

    let response = app
        .request(Method::GET, "/api/v1/orders", None, Some("invalid.token"))
        .await;

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
#[ignore = "requires SQLite and Redis integration environment"]
async fn test_auth_service_token_roundtrip() {
    let app = TestApp::new().await;
    let auth_service = app.auth_service();

    let user = User {
        id: Uuid::new_v4(),
        name: "Round Trip User".to_string(),
        email: "roundtrip@example.com".to_string(),
        password_hash: "".to_string(),
        tenant_id: None,
        active: true,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    let now = Utc::now();
    let claims = Claims {
        sub: user.id.to_string(),
        name: Some(user.name.clone()),
        email: Some(user.email.clone()),
        roles: vec!["user".to_string()],
        permissions: vec!["orders:read".to_string()],
        tenant_id: None,
        jti: Uuid::new_v4().to_string(),
        iat: now.timestamp(),
        exp: (now + chrono::Duration::minutes(30)).timestamp(),
        nbf: now.timestamp(),
        iss: "stateset-auth".to_string(),
        aud: "stateset-api".to_string(),
        scope: None,
    };

    let token = jsonwebtoken::encode(
        &jsonwebtoken::Header::new(jsonwebtoken::Algorithm::HS256),
        &claims,
        &jsonwebtoken::EncodingKey::from_secret(app.state.config.jwt_secret.as_bytes()),
    )
    .expect("encode access token");

    let claims = auth_service
        .validate_token(&token)
        .await
        .expect("validate token");

    assert_eq!(claims.sub, user.id.to_string());
    assert!(claims.roles.contains(&"user".to_string()));
}
