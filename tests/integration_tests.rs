mod common;

use axum::http::{Method, StatusCode};
use chrono::Utc;
use rust_decimal::Decimal;
use sea_orm::ActiveValue::Set;
use sea_orm::{ActiveModelTrait, EntityTrait};
use serde_json::{json, Value};
use uuid::Uuid;

use common::TestApp;
use stateset_api::entities::inventory_location;

#[allow(dead_code)]
fn assert_app_state_bounds() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<stateset_api::AppState>();
}

#[tokio::test]
#[ignore = "requires SQLite and Redis integration environment"]
async fn test_health_endpoint() {
    let app = TestApp::new().await;

    let response = app.request(Method::GET, "/health", None, None).await;

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
#[ignore = "requires SQLite and Redis integration environment"]
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
#[ignore = "requires SQLite and Redis integration environment"]
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
#[ignore = "requires SQLite and Redis integration environment"]
async fn test_inventory_management() {
    let app = TestApp::new().await;
    let db = app.state.db.clone();
    let location_id: i32 = 501;

    // Ensure supporting location exists for inventory operations
    if inventory_location::Entity::find_by_id(location_id)
        .one(db.as_ref())
        .await
        .unwrap()
        .is_none()
    {
        inventory_location::ActiveModel {
            location_id: Set(location_id),
            location_name: Set("Integration Warehouse".to_string()),
        }
        .insert(db.as_ref())
        .await
        .expect("insert test location");
    }

    // Test listing inventory
    let response = app
        .request_authenticated(Method::GET, "/api/v1/inventory", None)
        .await;

    assert_eq!(response.status(), StatusCode::OK);

    // Test creating inventory item using the supported payload
    let create_payload = json!({
        "item_number": "SKU-INTEGRATION-01",
        "description": "Integration test item",
        "primary_uom_code": "EA",
        "organization_id": 1,
        "location_id": location_id,
        "quantity_on_hand": 100,
        "reason": "Initial stock load"
    });

    let response = app
        .request_authenticated(
            Method::POST,
            "/api/v1/inventory",
            Some(create_payload.clone()),
        )
        .await;

    assert_eq!(response.status(), StatusCode::CREATED);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("read create body");
    let created_json: Value = serde_json::from_slice(&body).expect("parse inventory create");
    let inventory_item_id = created_json["data"]["inventory_item_id"]
        .as_i64()
        .expect("inventory id present");
    let inventory_path = format!("/api/v1/inventory/{}", inventory_item_id);

    // Test updating on-hand quantity
    let update_payload = json!({
        "location_id": location_id,
        "on_hand": 150,
        "reason": "Stock replenishment"
    });

    let response = app
        .request_authenticated(Method::PUT, &inventory_path, Some(update_payload))
        .await;

    assert_eq!(response.status(), StatusCode::OK);

    // Test reserving inventory (acts as allocation)
    let reserve_payload = json!({
        "location_id": location_id,
        "quantity": 10,
        "reference_id": Uuid::new_v4().to_string(),
        "reference_type": "integration_test"
    });

    let response = app
        .request_authenticated(
            Method::POST,
            &format!("/api/v1/inventory/{}/reserve", inventory_item_id),
            Some(reserve_payload),
        )
        .await;

    assert_eq!(response.status(), StatusCode::OK);

    // Test releasing a portion of the reservation
    let release_payload = json!({
        "location_id": location_id,
        "quantity": 5
    });

    let response = app
        .request_authenticated(
            Method::POST,
            &format!("/api/v1/inventory/{}/release", inventory_item_id),
            Some(release_payload),
        )
        .await;

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
#[ignore = "requires SQLite and Redis integration environment"]
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
#[ignore = "requires SQLite and Redis integration environment"]
async fn test_shipments_tracking() {
    let app = TestApp::new().await;

    let order_service = app.state.services.order.clone();
    let customer_id = Uuid::new_v4();
    let order = order_service
        .create_order_minimal(
            customer_id,
            Decimal::new(10_000, 2),
            Some("USD".to_string()),
            Some("shipment test order".to_string()),
            None,
            None,
            None,
        )
        .await
        .expect("create order for shipment test");
    let order_id = order.id;
    let tracking_number = "1Z123456789";
    // Test creating a shipment
    let create_payload = json!({
        "order_id": order_id,
        "shipping_address": "123 Main St, Anytown, CA 90210",
        "shipping_method": "standard",
        "tracking_number": tracking_number,
        "recipient_name": "Test Receiver"
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
#[ignore = "requires SQLite and Redis integration environment"]
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
#[ignore = "requires SQLite and Redis integration environment"]
async fn test_work_orders_manufacturing() {
    let app = TestApp::new().await;

    let create_payload = json!({
        "title": "Integration WO",
        "description": "Integration test work order",
        "priority": "normal",
        "status": "pending",
        "parts_required": {"component": 1}
    });

    let response = app
        .request_authenticated(Method::POST, "/api/v1/work-orders", Some(create_payload))
        .await;

    assert_eq!(response.status(), StatusCode::CREATED);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("read create work order");
    let create_json: Value = serde_json::from_slice(&body).expect("parse work order create");
    let work_order_id = create_json["data"]["id"]
        .as_str()
        .expect("work order id present")
        .to_string();

    // List work orders and verify seeded record is returned
    let response = app
        .request_authenticated(Method::GET, "/api/v1/work-orders?limit=10&offset=0", None)
        .await;

    assert_eq!(response.status(), StatusCode::OK);

    let list_body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("read work order list");
    let list_json: Value = serde_json::from_slice(&list_body).expect("parse work order list");
    let items = list_json["data"]["items"].as_array().expect("items array");
    assert!(
        items
            .iter()
            .any(|item| item["id"].as_str() == Some(work_order_id.as_str())),
        "expected work order id in list"
    );

    // Fetch the specific work order
    let response = app
        .request_authenticated(
            Method::GET,
            &format!("/api/v1/work-orders/{}", work_order_id),
            None,
        )
        .await;

    assert_eq!(response.status(), StatusCode::OK);

    let detail_body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("read work order detail");
    let detail_json: Value = serde_json::from_slice(&detail_body).expect("parse detail");
    assert_eq!(
        detail_json["data"]["id"].as_str(),
        Some(work_order_id.as_str()),
        "detail id mismatch"
    );
    assert_eq!(
        detail_json["data"]["title"], "Integration WO",
        "detail title mismatch"
    );
}

#[tokio::test]
#[ignore = "requires SQLite and Redis integration environment"]
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
#[ignore = "requires SQLite and Redis integration environment"]
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
