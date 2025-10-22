mod common;

use axum::{
    body,
    http::{Method, StatusCode},
};
use common::TestApp;
use serde_json::{json, Value};

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
        "customer_id": "cust-comprehensive",
        "items": [
            {
                "product_id": "prod-comprehensive",
                "quantity": 1,
                "price": 19.99
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
    assert_eq!(status, StatusCode::CREATED);
    let order_id = order_body["data"]["id"]
        .as_str()
        .expect("order id present")
        .to_string();

    let (status, _) = make_request(&app, Method::GET, "/api/v1/orders", None, true).await;
    assert_eq!(status, StatusCode::OK);

    let order_uri = format!("/api/v1/orders/{}", order_id);
    let (status, _) = make_request(&app, Method::GET, &order_uri, None, true).await;
    assert_eq!(status, StatusCode::OK);

    // Basic inventory read should also succeed.
    let (status, _) = make_request(&app, Method::GET, "/api/v1/inventory", None, true).await;
    assert_eq!(status, StatusCode::OK);
}
