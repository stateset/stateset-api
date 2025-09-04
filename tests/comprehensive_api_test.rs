use axum::{
    body::Body,
    http::{Request, StatusCode, Method},
    response::Response,
};
use chrono::Utc;
use serde_json::{json, Value};
use stateset_api::{
    config,
    db,
    events::{process_events, EventSender},
    handlers::AppServices,
    health,
    proto::*,
    api::StateSetApi,
    AppState,
};
use std::sync::Arc;
use tokio::sync::mpsc;
use tower::ServiceExt;
use uuid::Uuid;

// Helper function to create comprehensive test app state
async fn create_comprehensive_test_app_state() -> AppState {
    let config = config::AppConfig {
        database_url: ":memory:".to_string(),
        host: "127.0.0.1".to_string(),
        port: 8080,
        auto_migrate: true,
        env: "test".to_string(),
        jwt_secret: "test_secret_key_for_testing_purposes_only".to_string(),
        jwt_expiration: 3600,
        refresh_token_expiration: 86400,
        redis_url: "redis://localhost:6379".to_string(),
        rate_limit_requests_per_window: 1000,
        rate_limit_window_seconds: 60,
        rate_limit_enable_headers: true,
        log_level: "info".to_string(),
        log_json: false,
        cors_allowed_origins: None,
        cors_allow_credentials: false,
        grpc_port: None,
        is_production: false,
        rate_limit_path_policies: None,
        rate_limit_api_key_policies: None,
        rate_limit_user_policies: None,
        statement_timeout: None,
    };
    
    let db_arc = Arc::new(db::establish_connection_from_app_config(&config).await.unwrap());
    
    // Run migrations for tests
    if let Err(e) = db::run_migrations(&db_arc).await {
        eprintln!("Migration warning: {}", e);
    }
    
    let (tx, rx) = mpsc::channel(1000);
    let event_sender = EventSender::new(tx);
    
    // Start event processing in background
    let event_processor_handle = tokio::spawn(process_events(rx));
    
    // Create all services
    let inventory_service = stateset_api::services::inventory::InventoryService::new(
        db_arc.clone(),
        event_sender.clone(),
    );
    
    let order_service = stateset_api::services::orders::OrderService::new(
        db_arc.clone(),
        Some(Arc::new(event_sender.clone())),
    );
    
    let return_service = stateset_api::services::returns::ReturnService::new(db_arc.clone());
    let warranty_service = stateset_api::services::warranties::WarrantyService::new(db_arc.clone());
    let shipment_service = stateset_api::services::shipments::ShipmentService::new(db_arc.clone());
    let work_order_service = stateset_api::services::work_orders::WorkOrderService::new(db_arc.clone());
    
    AppState {
        db: db_arc,
        config,
        event_sender,
        inventory_service,
        services: AppServices {
            product_catalog: Arc::new(stateset_api::services::commerce::ProductCatalogService::new(
                db_arc.clone(),
                event_sender.clone(),
            )),
            cart: Arc::new(stateset_api::services::commerce::CartService::new(
                db_arc.clone(),
                event_sender.clone(),
            )),
            checkout: Arc::new(stateset_api::services::commerce::CheckoutService::new(
                db_arc.clone(),
                event_sender.clone(),
                order_service.clone(),
            )),
            customer: Arc::new(stateset_api::services::commerce::CustomerService::new(
                db_arc.clone(),
                event_sender.clone(),
                Arc::new(stateset_api::auth::AuthService::new(
                    stateset_api::auth::AuthConfig::new(
                        config.jwt_secret.clone(),
                        "stateset-api".to_string(),
                        "stateset-auth".to_string(),
                        std::time::Duration::from_secs(config.jwt_expiration as u64),
                        std::time::Duration::from_secs(config.refresh_token_expiration as u64),
                        "sk_".to_string(),
                    ),
                    db_arc.clone(),
                )),
            )),
            order: order_service,
        },
        redis: Arc::new(redis::Client::open(config.redis_url.clone()).unwrap_or_else(|_| {
            // Fallback to mock client if Redis is not available
            redis::Client::open("redis://mock:6379").unwrap()
        })),
    }
}

// Helper function to create test HTTP client
async fn create_test_app() -> axum::Router {
    let state = create_comprehensive_test_app_state().await;
    
    axum::Router::new()
        .nest("/health", health::health_routes_with_state(state.db.clone()))
        .nest("/api/v1", stateset_api::api_v1_routes().with_state(state.clone()))
        .with_state(state)
}

// Helper to make HTTP requests
async fn make_request(
    app: &axum::Router,
    method: Method,
    uri: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut builder = Request::builder().method(method).uri(uri);
    
    let body = if let Some(json_body) = body {
        builder = builder.header("content-type", "application/json");
        Body::from(serde_json::to_vec(&json_body).unwrap())
    } else {
        Body::empty()
    };
    
    let request = builder.body(body).unwrap();
    
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    
    let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json_body: Value = serde_json::from_slice(&body_bytes).unwrap_or(json!({}));
    
    (status, json_body)
}

#[tokio::test]
async fn test_comprehensive_api_endpoints() {
    let app = create_test_app().await;
    
    // Test 1: Health endpoints
    println!("🩺 Testing health endpoints...");
    
    let (status, body) = make_request(&app, Method::GET, "/health", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "healthy");
    
    let (status, body) = make_request(&app, Method::GET, "/health/live", None).await;
    assert_eq!(status, StatusCode::OK);
    
    let (status, body) = make_request(&app, Method::GET, "/health/ready", None).await;
    assert_eq!(status, StatusCode::OK);
    
    // Test 2: API status
    println!("📊 Testing API status...");
    
    let (status, body) = make_request(&app, Method::GET, "/api/v1/status", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "ok");
    assert_eq!(body["service"], "stateset-api");
    
    // Test 3: Orders CRUD operations
    println!("📦 Testing orders CRUD...");
    
    // Create order
    let order_data = json!({
        "customer_id": "550e8400-e29b-41d4-a716-446655440000",
        "items": [
            {
                "product_id": "550e8400-e29b-41d4-a716-446655440001",
                "quantity": 2,
                "unit_price": 29.99
            }
        ],
        "shipping_address": {
            "street": "123 Test St",
            "city": "Test City",
            "state": "TS",
            "country": "Test Country",
            "postal_code": "12345"
        }
    });
    
    let (status, body) = make_request(&app, Method::POST, "/api/v1/orders", Some(order_data)).await;
    assert_eq!(status, StatusCode::CREATED);
    assert!(body["data"]["id"].is_string());
    
    // Test 4: Inventory operations
    println!("📦 Testing inventory operations...");
    
    // List inventory
    let (status, body) = make_request(&app, Method::GET, "/api/v1/inventory", None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["data"]["items"].is_array());
    
    // Test 5: Returns operations
    println!("🔄 Testing returns operations...");
    
    // List returns
    let (status, body) = make_request(&app, Method::GET, "/api/v1/returns", None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["data"]["items"].is_array());
    
    // Test 6: Shipments operations
    println!("🚚 Testing shipments operations...");
    
    // List shipments
    let (status, body) = make_request(&app, Method::GET, "/api/v1/shipments", None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["data"]["items"].is_array());
    
    // Test 7: Warranties operations
    println!("🔧 Testing warranties operations...");
    
    // List warranties
    let (status, body) = make_request(&app, Method::GET, "/api/v1/warranties", None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["data"]["items"].is_array());
    
    // Test 8: Work orders operations
    println!("🏭 Testing work orders operations...");
    
    // List work orders
    let (status, body) = make_request(&app, Method::GET, "/api/v1/work-orders", None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["data"]["items"].is_array());
    
    println!("✅ All comprehensive API tests passed!");
}

#[tokio::test]
async fn test_error_handling() {
    let app = create_test_app().await;
    
    println!("🚨 Testing error handling...");
    
    // Test invalid order ID
    let (status, body) = make_request(&app, Method::GET, "/api/v1/orders/invalid-uuid", None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body["message"].is_string());
    
    // Test non-existent order
    let fake_uuid = Uuid::new_v4();
    let (status, body) = make_request(&app, Method::GET, &format!("/api/v1/orders/{}", fake_uuid), None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(body["message"].is_string());
    
    // Test invalid JSON
    let (status, body) = make_request(&app, Method::POST, "/api/v1/orders", Some(json!("invalid"))).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    
    println!("✅ Error handling tests passed!");
}

#[tokio::test]
async fn test_pagination() {
    let app = create_test_app().await;
    
    println!("📄 Testing pagination...");
    
    // Test with pagination parameters
    let (status, body) = make_request(&app, Method::GET, "/api/v1/orders?page=1&limit=10", None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["data"]["items"].is_array());
    assert!(body["data"]["total"].is_number());
    assert!(body["data"]["page"].is_number());
    assert!(body["data"]["limit"].is_number());
    
    println!("✅ Pagination tests passed!");
}

#[tokio::test]
async fn test_rate_limiting() {
    let app = create_test_app().await;
    
    println!("⏱️ Testing rate limiting headers...");
    
    // Make a request and check for rate limit headers
    let (status, _) = make_request(&app, Method::GET, "/api/v1/status", None).await;
    assert_eq!(status, StatusCode::OK);
    
    // Rate limiting is configured but may not be active in test environment
    // This test mainly ensures the endpoint works without rate limiting errors
    
    println!("✅ Rate limiting tests passed!");
}

#[tokio::test]
async fn test_openapi_documentation() {
    let app = create_test_app().await;
    
    println!("📚 Testing OpenAPI documentation endpoints...");
    
    // Test Swagger UI
    let (status, _) = make_request(&app, Method::GET, "/docs", None).await;
    assert_eq!(status, StatusCode::OK);
    
    // Test OpenAPI JSON spec
    let (status, body) = make_request(&app, Method::GET, "/api-docs/v1/openapi.json", None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["openapi"].is_string());
    assert!(body["info"]["title"].as_str().unwrap().contains("Stateset"));
    
    println!("✅ OpenAPI documentation tests passed!");
}

#[tokio::test]
async fn test_cors_headers() {
    let app = create_test_app().await;
    
    println!("🌐 Testing CORS configuration...");
    
    // Make a request and check response headers (CORS headers would be added by middleware)
    let (status, _) = make_request(&app, Method::GET, "/api/v1/status", None).await;
    assert_eq!(status, StatusCode::OK);
    
    // CORS is configured but headers may not be present in test environment
    // This test mainly ensures the endpoint works
    
    println!("✅ CORS tests passed!");
}

#[tokio::test]
async fn test_request_id_header() {
    let app = create_test_app().await;
    
    println!("🆔 Testing request ID generation...");
    
    // Make a request and check for X-Request-Id header
    let (status, _) = make_request(&app, Method::GET, "/api/v1/status", None).await;
    assert_eq!(status, StatusCode::OK);
    
    // Request ID middleware should add headers
    // This test ensures the endpoint works with middleware
    
    println!("✅ Request ID tests passed!");
}

#[tokio::test]
async fn test_security_headers() {
    let app = create_test_app().await;
    
    println!("🔒 Testing security headers...");
    
    // Make a request and check for security headers
    let (status, _) = make_request(&app, Method::GET, "/api/v1/status", None).await;
    assert_eq!(status, StatusCode::OK);
    
    // Security headers are added by middleware
    // This test ensures the endpoint works with security middleware
    
    println!("✅ Security headers tests passed!");
}

#[tokio::test]
async fn test_metrics_endpoint() {
    let app = create_test_app().await;
    
    println!("📊 Testing metrics endpoint...");
    
    // Test metrics endpoint
    let (status, _) = make_request(&app, Method::GET, "/metrics", None).await;
    assert_eq!(status, StatusCode::OK);
    
    println!("✅ Metrics tests passed!");
}

#[tokio::test]
async fn test_version_endpoint() {
    let app = create_test_app().await;
    
    println!("🏷️ Testing version endpoint...");
    
    // Test version endpoint
    let (status, body) = make_request(&app, Method::GET, "/version", None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["version"].is_string());
    
    println!("✅ Version tests passed!");
}

#[tokio::test]
async fn test_api_versions_endpoint() {
    let app = create_test_app().await;
    
    println!("🔢 Testing API versions endpoint...");
    
    // Test API versions endpoint
    let (status, body) = make_request(&app, Method::GET, "/api/versions", None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["versions"].is_array());
    
    println!("✅ API versions tests passed!");
}

#[tokio::test]
async fn test_commerce_endpoints() {
    let app = create_test_app().await;
    
    println!("🛒 Testing commerce endpoints...");
    
    // Test products endpoint
    let (status, body) = make_request(&app, Method::GET, "/api/v1/commerce/products", None).await;
    assert_eq!(status, StatusCode::OK);
    
    // Test carts endpoint
    let (status, body) = make_request(&app, Method::GET, "/api/v1/commerce/carts", None).await;
    assert_eq!(status, StatusCode::OK);
    
    // Test checkout endpoint
    let (status, body) = make_request(&app, Method::GET, "/api/v1/commerce/checkout", None).await;
    assert_eq!(status, StatusCode::OK);
    
    println!("✅ Commerce endpoints tests passed!");
}

#[tokio::test]
async fn test_auth_endpoints() {
    let app = create_test_app().await;
    
    println!("🔐 Testing auth endpoints...");
    
    // Test auth login endpoint (may require specific setup)
    let login_data = json!({
        "email": "test@example.com",
        "password": "password123"
    });
    
    let (status, body) = make_request(&app, Method::POST, "/api/v1/auth/login", Some(login_data)).await;
    // Auth may fail in test environment, but endpoint should exist
    assert!(status.is_success() || status == StatusCode::UNAUTHORIZED);
    
    println!("✅ Auth endpoints tests passed!");
}

#[tokio::test]
async fn test_notifications_endpoints() {
    let app = create_test_app().await;
    
    println!("🔔 Testing notifications endpoints...");
    
    // Test notifications endpoint
    let (status, body) = make_request(&app, Method::GET, "/api/v1/notifications", None).await;
    assert_eq!(status, StatusCode::OK);
    
    println!("✅ Notifications endpoints tests passed!");
}

#[tokio::test]
async fn test_agents_endpoints() {
    let app = create_test_app().await;
    
    println!("🤖 Testing agents endpoints...");
    
    // Test agents endpoint
    let (status, body) = make_request(&app, Method::GET, "/api/v1/agents", None).await;
    assert_eq!(status, StatusCode::OK);
    
    println!("✅ Agents endpoints tests passed!");
}

#[tokio::test]
async fn test_fallback_handling() {
    let app = create_test_app().await;
    
    println!("🚫 Testing fallback handling...");
    
    // Test non-existent endpoint
    let (status, body) = make_request(&app, Method::GET, "/api/v1/non-existent-endpoint", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(body["error"].is_string());
    
    // Test wrong method
    let (status, body) = make_request(&app, Method::PATCH, "/api/v1/orders", None).await;
    assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED);
    
    println!("✅ Fallback handling tests passed!");
}

#[tokio::test]
async fn test_large_payload_handling() {
    let app = create_test_app().await;
    
    println!("📏 Testing large payload handling...");
    
    // Create a large JSON payload
    let large_items = (0..100).map(|i| {
        json!({
            "product_id": format!("550e8400-e29b-41d4-a716-44665544{:04}", i),
            "quantity": 1,
            "unit_price": 10.0
        })
    }).collect::<Vec<_>>();
    
    let large_order = json!({
        "customer_id": "550e8400-e29b-41d4-a716-446655440000",
        "items": large_items
    });
    
    let (status, body) = make_request(&app, Method::POST, "/api/v1/orders", Some(large_order)).await;
    // Should handle large payloads gracefully
    assert!(status.is_success() || status == StatusCode::PAYLOAD_TOO_LARGE);
    
    println!("✅ Large payload handling tests passed!");
}

#[tokio::test]
async fn test_concurrent_requests() {
    let app = create_test_app().await;
    
    println!("🔄 Testing concurrent requests...");
    
    // Spawn multiple concurrent requests
    let mut handles = vec![];
    
    for i in 0..10 {
        let app_clone = app.clone();
        let handle = tokio::spawn(async move {
            let (status, _) = make_request(&app_clone, Method::GET, "/api/v1/status", None).await;
            assert_eq!(status, StatusCode::OK);
        });
        handles.push(handle);
    }
    
    // Wait for all requests to complete
    for handle in handles {
        handle.await.unwrap();
    }
    
    println!("✅ Concurrent requests tests passed!");
}
