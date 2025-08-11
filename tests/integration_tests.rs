use axum::{
    body::Body,
    http::{Request, StatusCode},
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
};
use std::sync::Arc;
use tokio::sync::mpsc;
use tower::ServiceExt;

// Helper function to create test app state
async fn create_test_app_state() -> stateset_api::AppState {
    let config = config::AppConfig {
        database_url: ":memory:".to_string(),
        host: "127.0.0.1".to_string(),
        port: 8080,
        auto_migrate: false,
        env: "test".to_string(),
    };
    
    let db_arc = Arc::new(db::establish_connection(&config.database_url).await.unwrap());
    let (tx, _rx) = mpsc::channel(1000);
    let event_sender = EventSender::new(tx);
    
    // Create inventory service for tests
    let inventory_service = stateset_api::services::inventory::InventoryService::new(
        db_arc.clone(),
        event_sender.clone(),
    );
    
    stateset_api::AppState {
        db: db_arc,
        config,
        event_sender,
        inventory_service,
    }
}

// Helper function to create test HTTP client
async fn create_test_app() -> axum::Router {
    let state = create_test_app_state().await;
    
    axum::Router::new()
        .nest("/health", health::health_routes())
        .nest("/api/v1", stateset_api::api_v1_routes().with_state(state.clone()))
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
    
    let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
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
    
    // Test creating a return
    let create_payload = json!({
        "order_id": "order_123",
        "reason": "defective",
        "description": "Item arrived damaged",
        "return_type": "refund",
        "items": [
            {
                "order_item_id": "item_123",
                "quantity": 1,
                "reason": "damaged"
            }
        ]
    });
    
    let response = app
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

    assert_eq!(response.status(), StatusCode::CREATED);
    
    // Test return approval
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/returns/return_123/approve")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    
    // Test return restocking
    let restock_payload = json!({
        "return_id": "return_123",
        "location_id": "loc_warehouse_001",
        "items": [
            {
                "return_item_id": "ret_item_123",
                "quantity": 1,
                "condition": "damaged"
            }
        ]
    });
    
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/returns/return_123/restock")
                .header("content-type", "application/json")
                .body(Body::from(restock_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_shipments_tracking() {
    let app = create_test_app().await;
    
    // Test creating a shipment
    let create_payload = json!({
        "order_id": "order_123",
        "carrier": "UPS",
        "service_type": "Ground",
        "shipping_address": {
            "street1": "123 Main St",
            "city": "Anytown",
            "state": "CA",
            "postal_code": "90210",
            "country": "US"
        },
        "items": [
            {
                "order_item_id": "item_123",
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
    
    let response = app
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

    assert_eq!(response.status(), StatusCode::CREATED);
    
    // Test shipment tracking
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/shipments/ship_123/track")
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
                .uri("/api/v1/shipments/track/1Z123456789")
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
                .uri("/api/v1/shipments/ship_123/ship")
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
    
    // Test creating a warranty
    let create_payload = json!({
        "product_id": "prod_abc",
        "customer_id": "cust_123",
        "order_id": "order_123",
        "serial_number": "SN123456",
        "warranty_type": "limited",
        "duration_months": 12,
        "terms": "Standard limited warranty covering manufacturing defects",
        "coverage": ["manufacturing_defects", "parts"],
        "exclusions": ["accidental_damage", "wear_and_tear"]
    });
    
    let response = app
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

    assert_eq!(response.status(), StatusCode::CREATED);
    
    // Test creating a warranty claim
    let claim_payload = json!({
        "warranty_id": "warranty_123",
        "claim_type": "repair",
        "issue_description": "Device stopped working after 3 months of use"
    });
    
    let response = app
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

    assert_eq!(response.status(), StatusCode::CREATED);
    
    // Test approving a claim
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/warranties/claims/claim_123/approve")
                .body(Body::empty())
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
                .uri("/api/v1/warranties/warranty_123/extend")
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
