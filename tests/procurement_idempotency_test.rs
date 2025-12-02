mod common;

use axum::{
    body,
    http::{Method, StatusCode},
    response::Response,
};
use sea_orm::{EntityTrait, PaginatorTrait};
use serde_json::{json, Value};
use stateset_api::models::{asn_entity, purchase_order_entity};
use uuid::Uuid;

use common::TestApp;

async fn response_json(response: Response) -> Value {
    let bytes = body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("response body bytes");
    serde_json::from_slice(&bytes).expect("json response")
}

#[tokio::test]
#[ignore = "requires SQLite and Redis integration environment"]
async fn purchase_order_create_is_idempotent() {
    let app = TestApp::new().await;

    let supplier_id = Uuid::new_v4();
    let item_product_id = Uuid::new_v4();

    let payload = json!({
        "supplier_id": supplier_id,
        "expected_delivery_date": "2025-01-15",
        "shipping_address": {
            "street": "123 Supply St",
            "city": "Logistics",
            "state": "CA",
            "postal_code": "90001",
            "country": "US"
        },
        "items": [
            {
                "product_id": item_product_id,
                "quantity": 10,
                "unit_price": 12.5,
                "tax_rate": 0.05,
                "currency": "USD",
                "description": "Widget"
            }
        ],
        "payment_terms": "Net 30",
        "currency": "USD",
        "notes": "Test purchase order"
    });

    let headers = [("Idempotency-Key", "po-idem-key-1")];

    let first = app
        .request_authenticated_with_headers(
            Method::POST,
            "/api/v1/purchase-orders",
            Some(payload.clone()),
            &headers,
        )
        .await;

    assert_eq!(first.status(), StatusCode::CREATED);
    let body_first = response_json(first).await;
    let first_id = body_first
        .get("id")
        .and_then(Value::as_str)
        .expect("purchase order id present")
        .to_string();

    let second = app
        .request_authenticated_with_headers(
            Method::POST,
            "/api/v1/purchase-orders",
            Some(payload),
            &headers,
        )
        .await;

    assert_eq!(second.status(), StatusCode::CREATED);
    let body_second = response_json(second).await;

    assert_eq!(body_first, body_second, "idempotent responses should match");
    let second_id = body_second
        .get("id")
        .and_then(Value::as_str)
        .expect("purchase order id present on replay");
    assert_eq!(first_id, second_id, "expected identical purchase order id");

    let count = purchase_order_entity::Entity::find()
        .count(&*app.state.db)
        .await
        .expect("count purchase orders");
    assert_eq!(count, 1, "expected a single purchase order record");
}

#[tokio::test]
#[ignore = "requires SQLite and Redis integration environment"]
async fn asn_create_is_idempotent() {
    let app = TestApp::new().await;

    let payload = json!({
        "purchase_order_id": Uuid::new_v4(),
        "supplier_id": Uuid::new_v4(),
        "supplier_name": "Acme Supplies",
        "expected_delivery_date": "2025-02-01T00:00:00Z",
        "shipping_address": {
            "street": "500 Dock Rd",
            "city": "Harbor",
            "state": "WA",
            "postal_code": "98101",
            "country": "US"
        },
        "carrier": {
            "carrier_name": "DHL",
            "tracking_number": "TRACK-123",
            "service_level": "Express"
        },
        "items": [
            {
                "product_id": Uuid::new_v4(),
                "product_name": "Widget A",
                "product_sku": "WIDGET-A",
                "quantity": 5,
                "unit_price": 19.99
            }
        ],
        "packages": [
            {
                "package_number": "PKG-1",
                "weight": 5.5,
                "weight_unit": "kg",
                "dimensions": {
                    "length": 10.0,
                    "width": 8.0,
                    "height": 6.0,
                    "unit": "cm"
                }
            }
        ]
    });

    let headers = [("Idempotency-Key", "asn-idem-key-1")];

    let first = app
        .request_authenticated_with_headers(
            Method::POST,
            "/api/v1/asns",
            Some(payload.clone()),
            &headers,
        )
        .await;

    assert_eq!(first.status(), StatusCode::CREATED);
    let body_first = response_json(first).await;
    let first_id = body_first
        .get("id")
        .and_then(Value::as_str)
        .expect("asn id present")
        .to_string();

    let second = app
        .request_authenticated_with_headers(Method::POST, "/api/v1/asns", Some(payload), &headers)
        .await;

    assert_eq!(second.status(), StatusCode::CREATED);
    let body_second = response_json(second).await;

    assert_eq!(body_first, body_second, "idempotent responses should match");
    let second_id = body_second
        .get("id")
        .and_then(Value::as_str)
        .expect("asn id present on replay");
    assert_eq!(first_id, second_id, "expected identical ASN id");

    let count = asn_entity::Entity::find()
        .count(&*app.state.db)
        .await
        .expect("count asns");
    assert_eq!(count, 1, "expected a single ASN record");
}
