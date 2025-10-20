mod common;

use axum::http::{Method, StatusCode};
use chrono::Utc;
use serde_json::json;
use stateset_api::auth::User;
use uuid::Uuid;

use common::TestApp;

#[tokio::test]
async fn test_protected_endpoint_requires_auth() {
    let app = TestApp::new().await;

    let response = app.request(Method::GET, "/api/v1/orders", None, None).await;

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_valid_token_allows_access() {
    let app = TestApp::new().await;

    let response = app
        .request_authenticated(Method::GET, "/api/v1/orders", None)
        .await;

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_invalid_token_rejected() {
    let app = TestApp::new().await;

    let response = app
        .request(Method::GET, "/api/v1/orders", None, Some("invalid.token"))
        .await;

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
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

    let tokens = auth_service
        .generate_token(&user)
        .await
        .expect("generate token");
    let claims = auth_service
        .validate_token(&tokens.access_token)
        .await
        .expect("validate token");

    assert_eq!(claims.sub, user.id.to_string());
    assert!(claims.roles.contains(&"user".to_string()));
}
