mod common;

use axum::{
    body,
    http::{Method, StatusCode},
};
use common::TestApp;
use rust_decimal::Decimal;
use sea_orm::EntityTrait;
use serde_json::{json, Value};
use stateset_api::entities::order::Entity as OrderEntity;
use uuid::Uuid;

async fn make_request(
    app: &TestApp,
    method: Method,
    uri: &str,
    body: Option<Value>,
    authenticated: bool,
) -> (StatusCode, Value) {
    let response = if authenticated {
        app.request_authenticated(method, uri, body).await
    } else {
        app.request(method, uri, body, None).await
    };

    let status = response.status();
    let bytes = body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let mut json_body = serde_json::from_slice(&bytes).unwrap_or_else(|_| json!({}));

    if let Value::Object(ref mut top_level) = json_body {
        if let Some(Value::Object(data_obj)) = top_level.get("data").cloned() {
            for (key, value) in data_obj {
                top_level.entry(key).or_insert(value);
            }
        }
    }

    (status, json_body)
}

#[tokio::test]
async fn comprehensive_smoke_test() {
    let app = TestApp::new().await;
    let variant = app
        .seed_product_variant("COMPREHENSIVE-SKU", Decimal::new(1999, 2))
        .await;
    let customer_id = Uuid::new_v4().to_string();

    // Health endpoints should respond without authentication.
    let (status, body) = make_request(&app, Method::GET, "/health", None, false).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "up");

    let (status, _) = make_request(&app, Method::GET, "/health/live", None, false).await;
    assert_eq!(status, StatusCode::OK);

    let (status, _) = make_request(&app, Method::GET, "/health/ready", None, false).await;
    assert_eq!(status, StatusCode::OK);

    // API status endpoint requires auth middleware to pass token.
    let (status, body) = make_request(&app, Method::GET, "/api/v1/status", None, true).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "ok");
    assert_eq!(body["service"], "stateset-api");

    // Create an order and ensure it can be fetched.
    let order_payload = json!({
        "customer_id": customer_id,
        "items": [
            {
                "product_id": variant.id.to_string(),
                "quantity": 1
            }
        ],
        "notes": "comprehensive smoke test"
    });

    let (status, order_body) = make_request(
        &app,
        Method::POST,
        "/api/v1/orders",
        Some(order_payload.clone()),
        true,
    )
    .await;
    if status != StatusCode::CREATED {
        panic!("create order failed (status {}): {}", status, order_body);
    }
    let order_id = order_body["data"]["id"]
        .as_str()
        .expect("order id present")
        .to_string();
    let order_number = order_body["data"]["order_number"]
        .as_str()
        .expect("order number present")
        .to_string();

    let (status, orders_list) = make_request(&app, Method::GET, "/api/v1/orders", None, true).await;
    assert_eq!(
        orders_list["items"][0]["id"].as_str(),
        Some(order_id.as_str())
    );

    let service_order = app
        .state
        .services
        .order
        .get_order(order_id.parse().expect("uuid"))
        .await
        .expect("order service call");
    assert!(service_order.is_some());

    let service_by_number = app
        .state
        .services
        .order
        .get_order_by_order_number(&order_number)
        .await
        .expect("order by number");
    assert!(service_by_number.is_some());

    let stored_orders = OrderEntity::find()
        .all(app.state.db.as_ref())
        .await
        .expect("load orders from db");
    assert!(stored_orders
        .iter()
        .any(|stored| stored.id.to_string() == order_id));
    assert_eq!(status, StatusCode::OK);

    // Basic inventory read should also succeed.
    let (status, _) = make_request(&app, Method::GET, "/api/v1/inventory", None, true).await;
    assert_eq!(status, StatusCode::OK);
}
