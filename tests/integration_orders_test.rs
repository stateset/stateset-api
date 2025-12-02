mod common;

use axum::{
    body,
    http::{Method, StatusCode},
};
use rust_decimal::Decimal;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde_json::{json, Value};
use stateset_api::{
    entities::{
        order::{self, Entity as OrderEntity},
        order_item::{Column as OrderItemColumn, Entity as OrderItemEntity},
    },
    services::orders::UpdateOrderStatusRequest as ServiceUpdateOrderStatusRequest,
};
use std::str::FromStr;
use uuid::Uuid;

use common::TestApp;

#[tokio::test]
#[ignore = "requires SQLite and Redis integration environment"]
async fn test_create_order_endpoint() {
    let app = TestApp::new().await;

    let customer_id = Uuid::new_v4();
    let variant = app
        .seed_product_variant("SKU-INT-001", Decimal::new(4_999, 2))
        .await;

    let payload = json!({
        "customer_id": customer_id.to_string(),
        "items": [
            {
                "product_id": variant.id.to_string(),
                "quantity": 2,
                "unit_price": "49.99"
            }
        ],
        "notes": "Test order for integration testing"
    });

    let response = app
        .request_authenticated(Method::POST, "/api/v1/orders", Some(payload))
        .await;

    assert_eq!(response.status(), StatusCode::CREATED);

    let body_bytes = body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("read response body");
    let response_data: Value = serde_json::from_slice(&body_bytes).expect("parse response body");

    assert!(response_data["success"].as_bool().unwrap_or(false));
    let data = &response_data["data"];
    assert_eq!(data["customer_id"], customer_id.to_string());
    assert_eq!(data["items"].as_array().map(|a| a.len()).unwrap_or(0), 1);

    let saved_order = OrderEntity::find()
        .filter(order::Column::CustomerId.eq(customer_id))
        .one(&*app.state.db)
        .await
        .expect("query order")
        .expect("order should exist");
    assert_eq!(
        saved_order.total_amount,
        Decimal::from_str("99.98").unwrap()
    );

    let items = OrderItemEntity::find()
        .filter(OrderItemColumn::OrderId.eq(saved_order.id))
        .all(&*app.state.db)
        .await
        .expect("query order items");
    assert_eq!(items.len(), 1);
}

#[tokio::test]
#[ignore = "requires SQLite and Redis integration environment"]
async fn test_create_order_rejects_unknown_variant() {
    let app = TestApp::new().await;
    let customer_id = Uuid::new_v4();

    let payload = json!({
        "customer_id": customer_id.to_string(),
        "items": [
            {
                "product_id": Uuid::new_v4().to_string(),
                "quantity": 1,
                "unit_price": "10.00"
            }
        ]
    });

    let response = app
        .request_authenticated(Method::POST, "/api/v1/orders", Some(payload))
        .await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
#[ignore = "requires SQLite and Redis integration environment"]
async fn test_create_order_accepts_sku_identifier() {
    let app = TestApp::new().await;
    let customer_id = Uuid::new_v4();
    let variant = app
        .seed_product_variant("SKU-ALIAS-01", Decimal::new(2_499, 2))
        .await;

    let payload = json!({
        "customer_id": customer_id.to_string(),
        "items": [
            {
                "product_id": variant.sku,
                "quantity": 1,
                "unit_price": "24.99"
            }
        ]
    });

    let response = app
        .request_authenticated(Method::POST, "/api/v1/orders", Some(payload))
        .await;

    assert_eq!(response.status(), StatusCode::CREATED);
}

#[tokio::test]
#[ignore = "requires SQLite and Redis integration environment"]
async fn test_create_order_rejects_price_mismatch() {
    let app = TestApp::new().await;
    let customer_id = Uuid::new_v4();
    let variant = app
        .seed_product_variant("SKU-MISMATCH", Decimal::new(1_999, 2))
        .await;

    let payload = json!({
        "customer_id": customer_id.to_string(),
        "items": [
            {
                "product_id": variant.id.to_string(),
                "quantity": 1,
                "unit_price": "25.00"
            }
        ]
    });

    let response = app
        .request_authenticated(Method::POST, "/api/v1/orders", Some(payload))
        .await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
#[ignore = "requires SQLite and Redis integration environment"]
async fn test_update_order_status_to_confirmed() {
    let app = TestApp::new().await;
    let order_service = app.state.services.order.clone();

    let created = order_service
        .create_order_minimal(
            Uuid::new_v4(),
            Decimal::from_str("10.00").unwrap(),
            Some("USD".to_string()),
            None,
            None,
            None,
            None,
        )
        .await
        .expect("create order");

    let payload = json!({ "status": "confirmed" });

    let response = app
        .request_authenticated(
            Method::PUT,
            &format!("/api/v1/orders/{}/status", created.id),
            Some(payload),
        )
        .await;

    assert_eq!(response.status(), StatusCode::OK);

    let body_bytes = body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("read response body");
    let response_data: Value = serde_json::from_slice(&body_bytes).expect("parse response body");

    assert!(response_data["success"].as_bool().unwrap_or(false));
    let data = &response_data["data"];
    assert_eq!(data["status"], "confirmed");

    let persisted = order_service
        .get_order(created.id)
        .await
        .expect("fetch order")
        .expect("order should exist");
    assert_eq!(persisted.status, "confirmed");
}

#[tokio::test]
#[ignore = "requires SQLite and Redis integration environment"]
async fn test_get_order_endpoint() {
    let app = TestApp::new().await;
    let order_service = app.state.services.order.clone();

    let customer_id = Uuid::new_v4();
    let created = order_service
        .create_order_minimal(
            customer_id,
            Decimal::from_str("149.99").unwrap(),
            Some("USD".to_string()),
            Some("integration test order".to_string()),
            None,
            None,
            None,
        )
        .await
        .expect("create order");

    let response = app
        .request_authenticated(Method::GET, &format!("/api/v1/orders/{}", created.id), None)
        .await;

    assert_eq!(response.status(), StatusCode::OK);

    let body_bytes = body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("read response body");
    let response_data: Value = serde_json::from_slice(&body_bytes).expect("parse response body");

    assert!(response_data["success"].as_bool().unwrap_or(false));
    let data = &response_data["data"];
    assert_eq!(data["id"], created.id.to_string());
    assert_eq!(data["customer_id"], customer_id.to_string());
    let total_amount = Decimal::from_str(
        data["total_amount"]
            .as_str()
            .expect("total amount should be a string"),
    )
    .unwrap();
    assert_eq!(total_amount, Decimal::from_str("149.99").unwrap());
}

#[tokio::test]
#[ignore = "requires SQLite and Redis integration environment"]
async fn test_list_orders_endpoint() {
    let app = TestApp::new().await;
    let order_service = app.state.services.order.clone();

    for i in 0..5 {
        order_service
            .create_order_minimal(
                Uuid::new_v4(),
                Decimal::from_str(&format!("1{}.00", i)).unwrap(),
                Some("USD".to_string()),
                Some(format!("seed order {}", i)),
                None,
                None,
                None,
            )
            .await
            .expect("seed orders");
    }

    let response = app
        .request_authenticated(Method::GET, "/api/v1/orders?page=1&limit=3", None)
        .await;

    assert_eq!(response.status(), StatusCode::OK);

    let body_bytes = body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("read response body");
    let response_data: Value = serde_json::from_slice(&body_bytes).expect("parse response body");

    assert!(response_data["success"].as_bool().unwrap_or(false));
    let data = &response_data["data"];
    assert_eq!(data["page"], 1);
    assert_eq!(data["limit"], 3);
    assert_eq!(data["items"].as_array().map(|a| a.len()).unwrap_or(0), 3);
    assert!(data["total"].as_u64().unwrap_or(0) >= 5);

    let first = data["items"]
        .as_array()
        .and_then(|arr| arr.first())
        .cloned()
        .expect("items present");
    assert!(first["order_number"].as_str().is_some());
    assert!(first["payment_status"].as_str().is_some());
    assert!(first["fulfillment_status"].as_str().is_some());
    assert!(first["items"].is_array());
}

#[tokio::test]
#[ignore = "requires SQLite and Redis integration environment"]
async fn test_orders_list_filters_and_search() {
    let app = TestApp::new().await;
    let order_service = app.state.services.order.clone();

    let demo_id = order_service
        .ensure_demo_order()
        .await
        .expect("ensure demo order");

    order_service
        .update_order_status(
            demo_id,
            ServiceUpdateOrderStatusRequest {
                status: "shipped".to_string(),
                notes: Some("integration test".to_string()),
            },
        )
        .await
        .expect("update demo status");

    for amount in [
        Decimal::new(2_500, 2),
        Decimal::new(1_500, 2),
        Decimal::new(7_500, 2),
    ] {
        let _ = order_service
            .create_order_minimal(
                Uuid::new_v4(),
                amount,
                Some("USD".to_string()),
                None,
                None,
                None,
                None,
            )
            .await
            .expect("seed order with amount");
    }

    let response = app
        .request_authenticated(Method::GET, "/api/v1/orders?status=shipped&limit=10", None)
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    let body_bytes = body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("read response body");
    let response_data: Value = serde_json::from_slice(&body_bytes).expect("parse response body");
    let items = response_data["data"]["items"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    assert!(!items.is_empty());
    assert!(items
        .iter()
        .all(|item| item["status"].as_str() == Some("shipped")));

    let response = app
        .request_authenticated(
            Method::GET,
            "/api/v1/orders?search=order_123&include_items=true&limit=5",
            None,
        )
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    let body_bytes = body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("read response body");
    let response_data: Value = serde_json::from_slice(&body_bytes).expect("parse response body");
    let data = &response_data["data"];
    let items = data["items"].as_array().cloned().unwrap_or_default();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["order_number"], "order_123");
    assert!(
        items[0]["items"]
            .as_array()
            .map(|arr| arr.len())
            .unwrap_or(0)
            >= 1
    );

    let response = app
        .request_authenticated(
            Method::GET,
            "/api/v1/orders?sort_by=total_amount&sort_order=asc&limit=10",
            None,
        )
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    let body_bytes = body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("read response body");
    let response_data: Value = serde_json::from_slice(&body_bytes).expect("parse response body");
    let items_array = response_data["data"]["items"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    assert!(items_array.len() >= 3);

    let totals: Vec<Decimal> = items_array
        .iter()
        .filter_map(|item| item["total_amount"].as_str())
        .filter_map(|raw| Decimal::from_str(raw).ok())
        .collect();

    let mut sorted = totals.clone();
    sorted.sort();
    assert_eq!(totals, sorted);
}

#[tokio::test]
#[ignore = "requires SQLite and Redis integration environment"]
async fn test_update_order_status_endpoint() {
    let app = TestApp::new().await;
    let order_service = app.state.services.order.clone();

    let created = order_service
        .create_order_minimal(
            Uuid::new_v4(),
            Decimal::from_str("75.50").unwrap(),
            Some("USD".to_string()),
            None,
            None,
            None,
            None,
        )
        .await
        .expect("create order");

    let payload = json!({
        "status": "processing",
        "reason": "Order is now being processed"
    });

    let response = app
        .request_authenticated(
            Method::PUT,
            &format!("/api/v1/orders/{}/status", created.id),
            Some(payload),
        )
        .await;

    assert_eq!(response.status(), StatusCode::OK);

    let body_bytes = body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("read response body");
    let response_data: Value = serde_json::from_slice(&body_bytes).expect("parse response body");

    assert!(response_data["success"].as_bool().unwrap_or(false));
    assert_eq!(response_data["data"]["status"], "processing");

    let updated_order = OrderEntity::find_by_id(created.id)
        .one(&*app.state.db)
        .await
        .expect("query updated order")
        .expect("updated order exists");
    assert_eq!(updated_order.status, "processing");
}
