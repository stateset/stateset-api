use axum::{
    body::Body,
    http::{Request, StatusCode},
    response::Response,
};
use chrono::Utc;
use redis::Client;
use serde_json::{json, Value};
use stateset_api::{
    api::StateSetApi,
    auth::{AuthConfig, AuthService},
    config::{self, AppConfig},
    db,
    events::{self, process_events, EventSender},
    handlers::AppServices,
    health,
    proto::*,
};
use std::sync::Arc;
use tokio::sync::mpsc;
use tower::ServiceExt;
use uuid::Uuid;

// Helper function to create test app state
async fn create_test_app_state() -> stateset_api::AppState {
    let mut config = AppConfig::new(
        "sqlite::memory:?cache=shared".to_string(),
        "redis://127.0.0.1:6379".to_string(),
        "test_secret_key_for_testing_purposes_only_32chars".to_string(),
        3600,
        86_400,
        "127.0.0.1".to_string(),
        18_080,
        "test".to_string(),
    );
    config.auto_migrate = true;

    let pool = db::establish_connection_from_app_config(&config)
        .await
        .expect("failed to create test database");
    if config.auto_migrate {
        db::run_migrations(&pool)
            .await
            .expect("failed to run migrations in tests");
    }

    let db_arc = Arc::new(pool);
    let (tx, rx) = mpsc::channel(1024);
    let event_sender = EventSender::new(tx);
    let _event_task = tokio::spawn(process_events(rx));

    let inventory_service = stateset_api::services::inventory::InventoryService::new(
        db_arc.clone(),
        event_sender.clone(),
    );

    let redis_client =
        Arc::new(Client::open(config.redis_url.clone()).expect("invalid redis url for tests"));

    let auth_cfg = AuthConfig::new(
        config.jwt_secret.clone(),
        "stateset-api".to_string(),
        "stateset-auth".to_string(),
        std::time::Duration::from_secs(config.jwt_expiration as u64),
        std::time::Duration::from_secs(config.refresh_token_expiration as u64),
        "sk_".to_string(),
    );
    let auth_service = Arc::new(AuthService::new(auth_cfg, db_arc.clone()));

    let services = AppServices::new(
        db_arc.clone(),
        Arc::new(event_sender.clone()),
        redis_client.clone(),
        auth_service,
    );

    stateset_api::AppState {
        db: db_arc,
        config,
        event_sender,
        inventory_service,
        services,
        redis: redis_client,
    }
}

// Helper function to create test HTTP client
async fn create_test_app() -> axum::Router {
    let state = create_test_app_state().await;

    axum::Router::new()
        .nest("/health", health::health_routes())
        .nest(
            "/api/v1",
            stateset_api::api_v1_routes().with_state(state.clone()),
        )
        .with_state(state)
}

#[tokio::test]
async fn test_health_endpoint() {
    let app = create_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_api_status() {
    let app = create_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/status")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["status"], "ok");
    assert_eq!(json["service"], "stateset-api");
}

#[tokio::test]
async fn test_orders_crud() {
    let app = create_test_app().await;

    // Test listing orders
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/orders")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    // Test creating an order
    let create_payload = json!({
        "customer_id": "cust_123",
        "items": [
            {
                "product_id": "prod_abc",
                "quantity": 2,
                "price": 49.99
            }
        ],
        "notes": "Test order"
    });

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/orders")
                .header("content-type", "application/json")
                .body(Body::from(create_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);

    // Test getting a specific order
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/orders/order_123")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    // Test updating an order
    let update_payload = json!({
        "status": "processing"
    });

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/v1/orders/order_123")
                .header("content-type", "application/json")
                .body(Body::from(update_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    // Test order actions
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/orders/order_123/cancel")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_inventory_management() {
    let app = create_test_app().await;

    // Test listing inventory
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/inventory")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    // Test creating inventory item
    let create_payload = json!({
        "product_id": "prod_abc",
        "location_id": "loc_warehouse_001",
        "quantity": 100,
        "unit_cost": 25.99
    });

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/inventory")
                .header("content-type", "application/json")
                .body(Body::from(create_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);

    // Test inventory adjustment
    let adjust_payload = json!({
        "adjustment_type": "increase",
        "quantity": 50,
        "reason": "Stock replenishment"
    });

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/inventory/adjust")
                .header("content-type", "application/json")
                .body(Body::from(adjust_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);

    // Test inventory allocation
    let allocate_payload = json!({
        "product_id": "prod_abc",
        "location_id": "loc_warehouse_001",
        "quantity": 10,
        "order_id": "order_123"
    });

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/inventory/allocate")
                .header("content-type", "application/json")
                .body(Body::from(allocate_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_returns_workflow() {
    let app = create_test_app().await;

    let order_id = Uuid::new_v4();
    // Test creating a return
    let create_payload = json!({
        "order_id": order_id,
        "reason": "defective",
        "description": "Item arrived damaged",
        "return_type": "refund",
        "items": [
            {
                "order_item_id": Uuid::new_v4(),
                "quantity": 1,
                "reason": "damaged"
            }
        ]
    });

    let response_create = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/returns")
                .header("content-type", "application/json")
                .body(Body::from(create_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response_create.status(), StatusCode::CREATED);
    let body_bytes = axum::body::to_bytes(response_create.into_body(), usize::MAX)
        .await
        .unwrap();
    let created_json: Value =
        serde_json::from_slice(&body_bytes).expect("return create response json");
    let return_id = created_json["data"]["id"]
        .as_str()
        .expect("return id present")
        .to_string();

    // Test return approval
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/returns/{}/approve", return_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    // Test return restocking
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/returns/{}/restock", return_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_shipments_tracking() {
    let app = create_test_app().await;

    let order_id = Uuid::new_v4();
    let tracking_number = "1Z123456789";
    // Test creating a shipment
    let create_payload = json!({
        "order_id": order_id,
        "carrier": "UPS",
        "service_type": "Ground",
        "tracking_number": tracking_number,
        "shipping_address": {
            "street1": "123 Main St",
            "city": "Anytown",
            "state": "CA",
            "postal_code": "90210",
            "country": "US"
        },
        "items": [
            {
                "order_item_id": Uuid::new_v4(),
                "quantity": 1
            }
        ],
        "weight": 2.5,
        "dimensions": {
            "length": 12.0,
            "width": 8.0,
            "height": 4.0,
            "unit": "in"
        }
    });

    let response_create = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/shipments")
                .header("content-type", "application/json")
                .body(Body::from(create_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response_create.status(), StatusCode::CREATED);
    let body_bytes = axum::body::to_bytes(response_create.into_body(), usize::MAX)
        .await
        .unwrap();
    let created_json: Value =
        serde_json::from_slice(&body_bytes).expect("shipment create response json");
    let shipment_id = created_json["data"]["id"]
        .as_str()
        .expect("shipment id present")
        .to_string();

    // Test shipment tracking
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/shipments/{}/track", shipment_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    // Test tracking by number
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/shipments/track/{}", tracking_number))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    // Test marking as shipped
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/shipments/{}/ship", shipment_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    // Test marking as delivered
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/shipments/{}/deliver", shipment_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_warranties_management() {
    let app = create_test_app().await;

    let product_id = Uuid::new_v4();
    let customer_id = Uuid::new_v4();
    let expiration_date = (Utc::now() + chrono::Duration::days(365)).to_rfc3339();
    // Test creating a warranty
    let create_payload = json!({
        "product_id": product_id,
        "customer_id": customer_id,
        "serial_number": "SN123456",
        "warranty_type": "limited",
        "expiration_date": expiration_date,
        "terms": "Standard limited warranty covering manufacturing defects"
    });

    let response_create = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/warranties")
                .header("content-type", "application/json")
                .body(Body::from(create_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response_create.status(), StatusCode::CREATED);
    let body_bytes = axum::body::to_bytes(response_create.into_body(), usize::MAX)
        .await
        .unwrap();
    let created_json: Value =
        serde_json::from_slice(&body_bytes).expect("warranty create response json");
    let warranty_id = created_json["data"]["id"]
        .as_str()
        .expect("warranty id present")
        .to_string();

    // Test creating a warranty claim
    let claim_payload = json!({
        "warranty_id": warranty_id,
        "customer_id": customer_id,
        "description": "Device stopped working after 3 months of use",
        "evidence": [],
        "contact_email": "customer@example.com"
    });

    let response_claim = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/warranties/claims")
                .header("content-type", "application/json")
                .body(Body::from(claim_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response_claim.status(), StatusCode::CREATED);
    let claim_bytes = axum::body::to_bytes(response_claim.into_body(), usize::MAX)
        .await
        .unwrap();
    let claim_json: Value =
        serde_json::from_slice(&claim_bytes).expect("warranty claim response json");
    let claim_id = claim_json["data"]["claim_id"]
        .as_str()
        .expect("claim id present")
        .to_string();

    // Test approving a claim
    let approve_payload = json!({
        "approved_by": Uuid::new_v4(),
        "resolution": "Approved for repair",
        "notes": "Customer eligible for repair service"
    });

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/warranties/claims/{}/approve", claim_id))
                .header("content-type", "application/json")
                .body(Body::from(approve_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    // Test extending warranty
    let extend_payload = json!({
        "additional_months": 6
    });

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/warranties/{}/extend", warranty_id))
                .header("content-type", "application/json")
                .body(Body::from(extend_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_work_orders_manufacturing() {
    let app = create_test_app().await;

    // Test creating a work order
    let create_payload = json!({
        "order_id": "order_123",
        "product_id": "prod_abc",
        "bom_id": "bom_123",
        "quantity": 100,
        "priority": "high",
        "work_center_id": "wc_assembly",
        "estimated_hours": 8.0,
        "notes": "High priority customer order"
    });

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/work-orders")
                .header("content-type", "application/json")
                .body(Body::from(create_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);

    // Test scheduling work order
    let schedule_payload = json!({
        "work_center_id": "wc_assembly",
        "scheduled_start": Utc::now(),
        "scheduled_end": Utc::now() + chrono::Duration::hours(8),
        "assigned_to": "worker_001"
    });

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/work-orders/wo_123/schedule")
                .header("content-type", "application/json")
                .body(Body::from(schedule_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    // Test starting work order
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/work-orders/wo_123/start")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    // Test material consumption
    let consume_payload = json!({
        "material_id": "mat_123",
        "quantity_consumed": 25.0,
        "notes": "Used for production"
    });

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/work-orders/wo_123/materials/mat_123/consume")
                .header("content-type", "application/json")
                .body(Body::from(consume_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    // Test completing work order
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/work-orders/wo_123/complete")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_filtering_and_pagination() {
    let app = create_test_app().await;

    // Test orders with filtering
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/orders?status=pending&limit=10&offset=0")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    // Test inventory with filtering
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/inventory?product_id=prod_abc&low_stock=true")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    // Test work orders with filtering
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/work-orders?status=in_progress&priority=high")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_error_handling() {
    let app = create_test_app().await;

    // Test invalid JSON
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/orders")
                .header("content-type", "application/json")
                .body(Body::from("invalid json"))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    // Test non-existent endpoint
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/nonexistent")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}
