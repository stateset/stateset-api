mod common;

use axum::http::{Method, StatusCode};
use chrono::Utc;
use serde_json::{json, Value};
use uuid::Uuid;

use common::TestApp;

#[allow(dead_code)]
fn assert_app_state_bounds() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<stateset_api::AppState>();
}

#[tokio::test]
async fn test_health_endpoint() {
    let app = TestApp::new().await;

    let response = app.request(Method::GET, "/health", None, None).await;

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_api_status() {
    let app = TestApp::new().await;

    let response = app
        .request_authenticated(Method::GET, "/api/v1/status", None)
        .await;

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
    let app = TestApp::new().await;
    let variant = app
        .seed_product_variant("SKU-CRUD-01", rust_decimal::Decimal::new(2_999, 2))
        .await;
    let customer_id = Uuid::new_v4();

    // Test listing orders
    let response = app
        .request_authenticated(Method::GET, "/api/v1/orders", None)
        .await;

    assert_eq!(response.status(), StatusCode::OK);

    // Test creating an order
    let create_payload = json!({
        "customer_id": customer_id.to_string(),
        "items": [
            {
                "product_id": variant.id.to_string(),
                "quantity": 2,
                "price": 29.99
            }
        ],
        "notes": "Test order"
    });

    let response = app
        .request_authenticated(Method::POST, "/api/v1/orders", Some(create_payload.clone()))
        .await;

    assert_eq!(response.status(), StatusCode::CREATED);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let create_json: Value = serde_json::from_slice(&body).unwrap();
    let order_id = create_json["data"]["id"]
        .as_str()
        .expect("order id present")
        .to_string();

    // Test getting a specific order
    let response = app
        .request_authenticated(Method::GET, &format!("/api/v1/orders/{}", order_id), None)
        .await;

    assert_eq!(response.status(), StatusCode::OK);

    // Test updating an order
    let update_payload = json!({
        "status": "processing"
    });

    let response = app
        .request_authenticated(
            Method::PUT,
            &format!("/api/v1/orders/{}", order_id),
            Some(update_payload),
        )
        .await;

    assert_eq!(response.status(), StatusCode::OK);

    // Test order actions
    let response = app
        .request_authenticated(
            Method::POST,
            &format!("/api/v1/orders/{}/cancel", order_id),
            None,
        )
        .await;

    assert_eq!(response.status(), StatusCode::OK);

    let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("read cancel body");
    let cancel_json: Value = serde_json::from_slice(&body_bytes).expect("parse cancel body");
    assert!(cancel_json["success"].as_bool().unwrap_or(false));
    assert_eq!(cancel_json["data"]["status"], "cancelled");
}

#[tokio::test]
async fn test_inventory_management() {
    let app = TestApp::new().await;

    // Test listing inventory
    let response = app
        .request_authenticated(Method::GET, "/api/v1/inventory", None)
        .await;

    assert_eq!(response.status(), StatusCode::OK);

    // Test creating inventory item
    let create_payload = json!({
        "product_id": "prod_abc",
        "location_id": "loc_warehouse_001",
        "quantity": 100,
        "unit_cost": 25.99
    });

    let response = app
        .request_authenticated(
            Method::POST,
            "/api/v1/inventory",
            Some(create_payload.clone()),
        )
        .await;

    assert_eq!(response.status(), StatusCode::CREATED);

    // Test inventory adjustment
    let adjust_payload = json!({
        "adjustment_type": "increase",
        "quantity": 50,
        "reason": "Stock replenishment"
    });

    let response = app
        .request_authenticated(
            Method::POST,
            "/api/v1/inventory/adjust",
            Some(adjust_payload),
        )
        .await;

    assert_eq!(response.status(), StatusCode::OK);

    // Test inventory allocation
    let allocate_payload = json!({
        "product_id": "prod_abc",
        "location_id": "loc_warehouse_001",
        "quantity": 10,
        "order_id": "order_123"
    });

    let response = app
        .request_authenticated(
            Method::POST,
            "/api/v1/inventory/allocate",
            Some(allocate_payload),
        )
        .await;

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_returns_workflow() {
    let app = TestApp::new().await;

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
        .request_authenticated(Method::POST, "/api/v1/returns", Some(create_payload))
        .await;

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
    let approve_uri = format!("/api/v1/returns/{}/approve", return_id);
    let response = app
        .request_authenticated(Method::POST, &approve_uri, None)
        .await;

    assert_eq!(response.status(), StatusCode::OK);

    // Test return restocking
    let restock_uri = format!("/api/v1/returns/{}/restock", return_id);
    let response = app
        .request_authenticated(Method::POST, &restock_uri, None)
        .await;

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_shipments_tracking() {
    let app = TestApp::new().await;

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
        .request_authenticated(Method::POST, "/api/v1/shipments", Some(create_payload))
        .await;

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
    let track_uri = format!("/api/v1/shipments/{}/track", shipment_id);
    let response = app
        .request_authenticated(Method::GET, &track_uri, None)
        .await;

    assert_eq!(response.status(), StatusCode::OK);

    // Test tracking by number
    let tracking_uri = format!("/api/v1/shipments/track/{}", tracking_number);
    let response = app
        .request_authenticated(Method::GET, &tracking_uri, None)
        .await;

    assert_eq!(response.status(), StatusCode::OK);

    // Test marking as shipped
    let ship_uri = format!("/api/v1/shipments/{}/ship", shipment_id);
    let response = app
        .request_authenticated(Method::POST, &ship_uri, None)
        .await;

    assert_eq!(response.status(), StatusCode::OK);

    // Test marking as delivered
    let deliver_uri = format!("/api/v1/shipments/{}/deliver", shipment_id);
    let response = app
        .request_authenticated(Method::POST, &deliver_uri, None)
        .await;

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_warranties_management() {
    let app = TestApp::new().await;

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
        .request_authenticated(Method::POST, "/api/v1/warranties", Some(create_payload))
        .await;

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
        .request_authenticated(
            Method::POST,
            "/api/v1/warranties/claims",
            Some(claim_payload),
        )
        .await;

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

    let approve_uri = format!("/api/v1/warranties/claims/{}/approve", claim_id);
    let response = app
        .request_authenticated(Method::POST, &approve_uri, Some(approve_payload))
        .await;

    assert_eq!(response.status(), StatusCode::OK);

    // Test extending warranty
    let extend_payload = json!({
        "additional_months": 6
    });

    let extend_uri = format!("/api/v1/warranties/{}/extend", warranty_id);
    let response = app
        .request_authenticated(Method::POST, &extend_uri, Some(extend_payload))
        .await;

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_work_orders_manufacturing() {
    let app = TestApp::new().await;

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
        .request_authenticated(Method::POST, "/api/v1/work-orders", Some(create_payload))
        .await;

    assert_eq!(response.status(), StatusCode::CREATED);

    // Test scheduling work order
    let schedule_payload = json!({
        "work_center_id": "wc_assembly",
        "scheduled_start": Utc::now(),
        "scheduled_end": Utc::now() + chrono::Duration::hours(8),
        "assigned_to": "worker_001"
    });

    let response = app
        .request_authenticated(
            Method::POST,
            "/api/v1/work-orders/wo_123/schedule",
            Some(schedule_payload),
        )
        .await;

    assert_eq!(response.status(), StatusCode::OK);

    // Test starting work order
    let response = app
        .request_authenticated(Method::POST, "/api/v1/work-orders/wo_123/start", None)
        .await;

    assert_eq!(response.status(), StatusCode::OK);

    // Test material consumption
    let consume_payload = json!({
        "material_id": "mat_123",
        "quantity_consumed": 25.0,
        "notes": "Used for production"
    });

    let response = app
        .request_authenticated(
            Method::POST,
            "/api/v1/work-orders/wo_123/materials/mat_123/consume",
            Some(consume_payload),
        )
        .await;

    assert_eq!(response.status(), StatusCode::OK);

    // Test completing work order
    let response = app
        .request_authenticated(Method::POST, "/api/v1/work-orders/wo_123/complete", None)
        .await;

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_filtering_and_pagination() {
    let app = TestApp::new().await;

    // Test orders with filtering
    let response = app
        .request_authenticated(
            Method::GET,
            "/api/v1/orders?status=pending&limit=10&offset=0",
            None,
        )
        .await;

    assert_eq!(response.status(), StatusCode::OK);

    // Test inventory with filtering
    let response = app
        .request_authenticated(
            Method::GET,
            "/api/v1/inventory?product_id=prod_abc&low_stock=true",
            None,
        )
        .await;

    assert_eq!(response.status(), StatusCode::OK);

    // Test work orders with filtering
    let response = app
        .request_authenticated(
            Method::GET,
            "/api/v1/work-orders?status=in_progress&priority=high",
            None,
        )
        .await;

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_error_handling() {
    let app = TestApp::new().await;

    // Test invalid JSON
    let response = app
        .request_authenticated(
            Method::POST,
            "/api/v1/orders",
            Some(Value::String("invalid json".into())),
        )
        .await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    // Test non-existent endpoint
    let response = app
        .request_authenticated(Method::GET, "/api/v1/nonexistent", None)
        .await;

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}
