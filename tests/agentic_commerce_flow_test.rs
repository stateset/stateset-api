mod common;

use axum::{
    body,
    http::{Method, StatusCode},
};
use rust_decimal::Decimal;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde_json::{json, Value};
use stateset_api::{
    entities::order::Entity as OrderEntity,
    models::{
        cash_sale::{Column as CashSaleColumn, Entity as CashSaleEntity},
        invoices::{Column as InvoiceColumn, Entity as InvoiceEntity},
        payment::{Column as PaymentColumn, Entity as PaymentEntity},
        shipment::{Column as ShipmentColumn, Entity as ShipmentEntity},
    },
};
use uuid::Uuid;

use common::TestApp;

#[tokio::test]
async fn agentic_checkout_end_to_end_persists_order_finance_and_fulfillment_records() {
    let app = TestApp::new().await;

    let variant = app
        .seed_product_variant("AGENTIC-E2E", Decimal::new(2_499, 2))
        .await;

    let buyer = json!({
        "first_name": "Ada",
        "last_name": "Lovelace",
        "email": "ada.lovelace@example.com",
        "phone_number": "+1-415-555-0101"
    });
    let address = json!({
        "name": "Ada Lovelace",
        "line_one": "123 Analytical Way",
        "line_two": "Suite 42",
        "city": "San Francisco",
        "state": "CA",
        "country": "US",
        "postal_code": "94105",
        "phone": "+1-415-555-0101",
        "email": "ada.lovelace@example.com"
    });

    // Start agentic checkout session
    let create_response = app
        .request_authenticated(
            Method::POST,
            "/api/v1/checkout_sessions",
            Some(json!({
                "buyer": buyer,
                "items": [{
                    "id": variant.id.to_string(),
                    "quantity": 2
                }],
                "fulfillment_address": address
            })),
        )
        .await;

    assert!(
        create_response.status() == StatusCode::CREATED
            || create_response.status() == StatusCode::OK
    );

    let create_body = body::to_bytes(create_response.into_body(), usize::MAX)
        .await
        .expect("read create session response");
    let create_session: Value =
        serde_json::from_slice(&create_body).expect("parse create session response");

    let session_id = create_session["id"]
        .as_str()
        .expect("session id")
        .to_string();
    let shipping_option_id = create_session["fulfillment_options"]
        .as_array()
        .expect("fulfillment options array")
        .iter()
        .find_map(|option| {
            (option["type"].as_str() == Some("shipping")).then(|| {
                option["id"]
                    .as_str()
                    .expect("shipping option id")
                    .to_string()
            })
        })
        .expect("shipping option available");

    // Select fulfillment option to make session ready for payment
    let update_response = app
        .request_authenticated(
            Method::POST,
            &format!("/api/v1/checkout_sessions/{}", session_id),
            Some(json!({
                "fulfillment_option_id": shipping_option_id
            })),
        )
        .await;
    assert_eq!(update_response.status(), StatusCode::OK);

    let update_body = body::to_bytes(update_response.into_body(), usize::MAX)
        .await
        .expect("read update session response");
    let update_session: Value =
        serde_json::from_slice(&update_body).expect("parse update session response");
    assert_eq!(update_session["status"].as_str(), Some("ready_for_payment"));

    // Complete checkout session
    let complete_payload = json!({
        "payment_data": {
            "token": "tok_test_agentic_e2e",
            "provider": "stripe",
            "billing_address": {
                "name": "Ada Lovelace",
                "line_one": "123 Analytical Way",
                "line_two": "Suite 42",
                "city": "San Francisco",
                "state": "CA",
                "country": "US",
                "postal_code": "94105",
                "phone": "+1-415-555-0101",
                "email": "ada.lovelace@example.com"
            }
        }
    });

    let complete_response = app
        .request_authenticated(
            Method::POST,
            &format!("/api/v1/checkout_sessions/{}/complete", session_id),
            Some(complete_payload),
        )
        .await;
    assert_eq!(complete_response.status(), StatusCode::OK);

    let complete_body = body::to_bytes(complete_response.into_body(), usize::MAX)
        .await
        .expect("read completion response");
    let complete_session: Value =
        serde_json::from_slice(&complete_body).expect("parse completion response");

    assert_eq!(complete_session["status"].as_str(), Some("completed"));
    let order_json = complete_session["order"]
        .as_object()
        .expect("order summary present");
    let order_id = Uuid::parse_str(
        order_json["id"]
            .as_str()
            .expect("order id present in response"),
    )
    .expect("valid order id");

    // Verify persisted order status and totals
    let order_record = OrderEntity::find_by_id(order_id)
        .one(&*app.state.db)
        .await
        .expect("load order from db")
        .expect("order exists after checkout");
    let normalized_payment_status = order_record.payment_status.to_lowercase();
    assert!(
        normalized_payment_status == "paid" || normalized_payment_status == "failed",
        "unexpected order payment status {}",
        order_record.payment_status
    );
    assert_eq!(order_record.status, "confirmed");
    assert_eq!(order_record.fulfillment_status.to_lowercase(), "processing");

    // Payment record
    let payment_record = PaymentEntity::find()
        .filter(PaymentColumn::OrderId.eq(order_id))
        .one(&*app.state.db)
        .await
        .expect("query payment")
        .expect("payment record exists");
    assert_eq!(payment_record.order_id, order_id);
    assert_eq!(payment_record.amount, order_record.total_amount);
    assert!(
        payment_record.status.to_lowercase() == "succeeded"
            || payment_record.status.to_lowercase() == "failed",
        "unexpected payment status {}",
        payment_record.status
    );

    // Invoice record
    let invoice_record = InvoiceEntity::find()
        .filter(InvoiceColumn::OrderId.eq(order_id.to_string()))
        .one(&*app.state.db)
        .await
        .expect("query invoice")
        .expect("invoice record exists");
    assert_eq!(
        order_json["invoice_id"].as_str(),
        Some(invoice_record.id.as_str())
    );

    // Cash sale record
    let cash_sale_record = CashSaleEntity::find()
        .filter(CashSaleColumn::OrderId.eq(order_id))
        .one(&*app.state.db)
        .await
        .expect("query cash sale")
        .expect("cash sale record exists");
    assert_eq!(cash_sale_record.amount, order_record.total_amount);

    // Shipment record
    let shipment_record = ShipmentEntity::find()
        .filter(ShipmentColumn::OrderId.eq(order_id))
        .one(&*app.state.db)
        .await
        .expect("query shipment")
        .expect("shipment record exists");
    let shipment_id_str = shipment_record.id.to_string();
    assert_eq!(
        order_json["shipment_id"].as_str(),
        Some(shipment_id_str.as_str())
    );
    assert!(!shipment_record.tracking_number.is_empty());

    // Fetch session from API and verify enrichment fields
    let session_get = app
        .request_authenticated(
            Method::GET,
            &format!("/api/v1/checkout_sessions/{}", session_id),
            None,
        )
        .await;
    assert_eq!(session_get.status(), StatusCode::OK);
    let session_body = body::to_bytes(session_get.into_body(), usize::MAX)
        .await
        .expect("read session body");
    let persisted_session: Value =
        serde_json::from_slice(&session_body).expect("parse session fetch");
    let order_id_str = order_id.to_string();
    let payment_id_str = payment_record.id.to_string();
    assert_eq!(
        persisted_session["order_id"].as_str(),
        Some(order_id_str.as_str())
    );
    assert_eq!(
        persisted_session["payment_id"].as_str(),
        Some(payment_id_str.as_str())
    );
    assert_eq!(
        persisted_session["invoice_id"].as_str(),
        Some(invoice_record.id.as_str())
    );
    assert_eq!(
        persisted_session["shipment_id"].as_str(),
        Some(shipment_id_str.as_str())
    );
}
