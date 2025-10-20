mod common;

use axum::{body, http::Method, response::Response};
use sea_orm::{ActiveModelTrait, Set};
use serde_json::{json, Value};
use stateset_api::entities::inventory_location;

use common::TestApp;

async fn response_json(response: Response) -> Value {
    let bytes = body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("response body bytes");
    serde_json::from_slice(&bytes).expect("json response")
}

#[tokio::test]
async fn inventory_item_lifecycle() {
    let app = TestApp::new().await;

    // Ensure a warehouse/location exists for the inventory operations
    let location = inventory_location::ActiveModel {
        location_name: Set("Main Warehouse".to_string()),
        ..Default::default()
    }
    .insert(app.state.db.as_ref())
    .await
    .expect("create location");
    let location_id = location.location_id;

    // Create inventory for a new item tied to item_master
    let create_payload = json!({
        "item_number": "TEST-ITEM-1",
        "description": "Widget",
        "primary_uom_code": "EA",
        "organization_id": 1,
        "location_id": location_id,
        "quantity_on_hand": 15,
        "reason": "initial load"
    });

    let response = app
        .request_authenticated(Method::POST, "/api/v1/inventory", Some(create_payload))
        .await;
    assert_eq!(response.status(), 201);
    let body = response_json(response).await;
    assert!(body["success"].as_bool().unwrap());
    let item = body["data"].clone();
    assert_eq!(item["item_number"], "TEST-ITEM-1");
    assert_eq!(item["quantities"]["available"], "15");
    let inventory_item_id = item["inventory_item_id"].as_i64().expect("item id");

    // Fetch the item by number
    let response = app
        .request_authenticated(Method::GET, "/api/v1/inventory/TEST-ITEM-1", None)
        .await;
    assert_eq!(response.status(), 200);
    let fetched = response_json(response).await;
    assert_eq!(fetched["success"], true);
    assert_eq!(fetched["data"]["quantities"]["available"], "15");

    // Update on-hand quantity to 20
    let update_payload = json!({
        "location_id": location_id,
        "on_hand": 20,
        "reason": "cycle count"
    });
    let response = app
        .request_authenticated(
            Method::PUT,
            "/api/v1/inventory/TEST-ITEM-1",
            Some(update_payload),
        )
        .await;
    assert_eq!(response.status(), 200);
    let updated = response_json(response).await;
    assert_eq!(updated["data"]["quantities"]["available"], "20");

    // Reserve 5 units
    let reserve_payload = json!({
        "location_id": location_id,
        "quantity": 5,
        "reference_type": "TEST"
    });
    let response = app
        .request_authenticated(
            Method::POST,
            "/api/v1/inventory/TEST-ITEM-1/reserve",
            Some(reserve_payload),
        )
        .await;
    assert_eq!(response.status(), 200);
    let reserved = response_json(response).await;
    assert!(reserved["data"]["reservation_id"].as_str().unwrap().len() > 0);
    assert_eq!(
        reserved["data"]["location"]["quantities"]["available"],
        "15"
    );

    // Release 3 units from reservation
    let release_payload = json!({
        "location_id": location_id,
        "quantity": 3
    });
    let response = app
        .request_authenticated(
            Method::POST,
            "/api/v1/inventory/TEST-ITEM-1/release",
            Some(release_payload),
        )
        .await;
    assert_eq!(response.status(), 200);
    let released = response_json(response).await;
    assert_eq!(released["data"]["quantities"]["available"], "18");

    // Low stock endpoint should include the item when threshold is above available quantity
    let uri = format!("/api/v1/inventory/low-stock?threshold=19&limit=50&offset=0");
    let response = app.request_authenticated(Method::GET, &uri, None).await;
    assert_eq!(response.status(), 200);
    let low_stock = response_json(response).await;
    assert_eq!(
        low_stock["data"]["items"][0]["inventory_item_id"],
        inventory_item_id
    );
}
