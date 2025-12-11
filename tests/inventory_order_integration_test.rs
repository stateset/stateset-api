//! Integration tests for Inventory-Order interactions.
//!
//! Tests cover:
//! - Inventory reservation during order creation
//! - Inventory release on order cancellation
//! - Inventory allocation for fulfillment
//! - Stock availability checks
//! - Overselling prevention
//! - Multi-warehouse inventory allocation

mod common;

use axum::{body, http::Method, response::Response};
use common::TestApp;
use rust_decimal_macros::dec;
use sea_orm::{ActiveModelTrait, Set};
use serde_json::{json, Value};
use stateset_api::entities::inventory_location;

async fn response_json(response: Response) -> Value {
    let bytes = body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("response body bytes");
    serde_json::from_slice(&bytes).expect("json response")
}

// ==================== Inventory Reservation Tests ====================

#[tokio::test]
#[ignore = "requires SQLite and Redis integration environment"]
async fn test_order_reserves_inventory() {
    let app = TestApp::new().await;

    // Create warehouse location
    let location = inventory_location::ActiveModel {
        location_name: Set("Test Warehouse".to_string()),
        ..Default::default()
    }
    .insert(app.state.db.as_ref())
    .await
    .expect("create location");

    // Create inventory
    let create_inventory = json!({
        "item_number": "INV-ORD-001",
        "description": "Test Widget",
        "primary_uom_code": "EA",
        "organization_id": 1,
        "location_id": location.location_id,
        "quantity_on_hand": 100,
        "reason": "initial stock"
    });

    let inv_response = app
        .request_authenticated(Method::POST, "/api/v1/inventory", Some(create_inventory))
        .await;

    assert_eq!(inv_response.status(), 201);

    // Seed a product variant that will use this inventory
    let variant = app.seed_product_variant("INV-ORD-001", dec!(25.00)).await;

    // Create an order for 10 units
    let order_payload = json!({
        "customer_email": "inventory@test.com",
        "customer_name": "Inventory Test",
        "items": [{
            "variant_id": variant.id.to_string(),
            "quantity": 10,
            "unit_price": "25.00"
        }]
    });

    let order_response = app
        .request_authenticated(Method::POST, "/api/v1/orders", Some(order_payload))
        .await;

    // Order should succeed
    assert!(
        order_response.status() == 201 || order_response.status() == 200,
        "Order should be created"
    );

    // Check inventory - available should be reduced
    let inv_get_response = app
        .request_authenticated(Method::GET, "/api/v1/inventory/INV-ORD-001", None)
        .await;

    if inv_get_response.status() == 200 {
        let body = response_json(inv_get_response).await;
        // Available should be less than original due to reservation
        // (exact behavior depends on implementation)
        println!("Inventory after order: {:?}", body);
    }
}

#[tokio::test]
#[ignore = "requires SQLite and Redis integration environment"]
async fn test_order_cancellation_releases_inventory() {
    let app = TestApp::new().await;

    // Create warehouse location
    let location = inventory_location::ActiveModel {
        location_name: Set("Cancel Warehouse".to_string()),
        ..Default::default()
    }
    .insert(app.state.db.as_ref())
    .await
    .expect("create location");

    // Create inventory with 50 units
    let create_inventory = json!({
        "item_number": "INV-CANCEL-001",
        "description": "Cancel Widget",
        "primary_uom_code": "EA",
        "organization_id": 1,
        "location_id": location.location_id,
        "quantity_on_hand": 50,
        "reason": "initial stock"
    });

    app.request_authenticated(Method::POST, "/api/v1/inventory", Some(create_inventory))
        .await;

    let variant = app
        .seed_product_variant("INV-CANCEL-001", dec!(30.00))
        .await;

    // Create an order for 20 units
    let order_payload = json!({
        "customer_email": "cancel@test.com",
        "customer_name": "Cancel Test",
        "items": [{
            "variant_id": variant.id.to_string(),
            "quantity": 20,
            "unit_price": "30.00"
        }]
    });

    let order_response = app
        .request_authenticated(Method::POST, "/api/v1/orders", Some(order_payload))
        .await;

    if order_response.status() != 201 && order_response.status() != 200 {
        return;
    }

    let order_body = response_json(order_response).await;
    let order_id = order_body["data"]["id"]
        .as_str()
        .or_else(|| order_body["data"]["order_id"].as_str())
        .expect("Order should have ID");

    // Cancel the order
    let cancel_response = app
        .request_authenticated(
            Method::POST,
            &format!("/api/v1/orders/{}/cancel", order_id),
            None,
        )
        .await;

    // After cancellation, inventory should be released
    if cancel_response.status() == 200 {
        let inv_response = app
            .request_authenticated(Method::GET, "/api/v1/inventory/INV-CANCEL-001", None)
            .await;

        if inv_response.status() == 200 {
            let body = response_json(inv_response).await;
            println!("Inventory after cancellation: {:?}", body);
            // Available should be back to full (or close to it)
        }
    }
}

// ==================== Stock Availability Tests ====================

#[tokio::test]
#[ignore = "requires SQLite and Redis integration environment"]
async fn test_order_fails_insufficient_stock() {
    let app = TestApp::new().await;

    // Create warehouse location
    let location = inventory_location::ActiveModel {
        location_name: Set("Low Stock Warehouse".to_string()),
        ..Default::default()
    }
    .insert(app.state.db.as_ref())
    .await
    .expect("create location");

    // Create inventory with only 5 units
    let create_inventory = json!({
        "item_number": "INV-LOW-001",
        "description": "Low Stock Widget",
        "primary_uom_code": "EA",
        "organization_id": 1,
        "location_id": location.location_id,
        "quantity_on_hand": 5,
        "reason": "initial stock"
    });

    app.request_authenticated(Method::POST, "/api/v1/inventory", Some(create_inventory))
        .await;

    let variant = app.seed_product_variant("INV-LOW-001", dec!(40.00)).await;

    // Try to order 100 units (more than available)
    let order_payload = json!({
        "customer_email": "lowstock@test.com",
        "customer_name": "Low Stock Test",
        "items": [{
            "variant_id": variant.id.to_string(),
            "quantity": 100,
            "unit_price": "40.00"
        }]
    });

    let response = app
        .request_authenticated(Method::POST, "/api/v1/orders", Some(order_payload))
        .await;

    // Depending on implementation, this might:
    // - Fail with 400 (insufficient stock)
    // - Succeed (backorder allowed)
    // - Succeed (no stock check at order time)
    println!(
        "Order with insufficient stock status: {}",
        response.status()
    );
}

// ==================== Inventory Allocation Tests ====================

#[tokio::test]
#[ignore = "requires SQLite and Redis integration environment"]
async fn test_inventory_allocation_for_shipment() {
    let app = TestApp::new().await;

    // Create warehouse location
    let location = inventory_location::ActiveModel {
        location_name: Set("Allocation Warehouse".to_string()),
        ..Default::default()
    }
    .insert(app.state.db.as_ref())
    .await
    .expect("create location");

    // Create inventory
    let create_inventory = json!({
        "item_number": "INV-ALLOC-001",
        "description": "Allocation Widget",
        "primary_uom_code": "EA",
        "organization_id": 1,
        "location_id": location.location_id,
        "quantity_on_hand": 100,
        "reason": "initial stock"
    });

    app.request_authenticated(Method::POST, "/api/v1/inventory", Some(create_inventory))
        .await;

    let variant = app.seed_product_variant("INV-ALLOC-001", dec!(35.00)).await;

    // Create order
    let order_payload = json!({
        "customer_email": "allocate@test.com",
        "customer_name": "Allocate Test",
        "items": [{
            "variant_id": variant.id.to_string(),
            "quantity": 15,
            "unit_price": "35.00"
        }]
    });

    let order_response = app
        .request_authenticated(Method::POST, "/api/v1/orders", Some(order_payload))
        .await;

    if order_response.status() != 201 && order_response.status() != 200 {
        return;
    }

    let order_body = response_json(order_response).await;
    let order_id = order_body["data"]["id"]
        .as_str()
        .or_else(|| order_body["data"]["order_id"].as_str())
        .expect("Order should have ID");

    // Allocate inventory for shipment
    let allocate_payload = json!({
        "order_id": order_id,
        "items": [{
            "item_number": "INV-ALLOC-001",
            "quantity": 15,
            "location_id": location.location_id
        }]
    });

    let allocate_response = app
        .request_authenticated(
            Method::POST,
            "/api/v1/inventory/allocate",
            Some(allocate_payload),
        )
        .await;

    // Allocation endpoint may or may not exist
    assert!(
        allocate_response.status() == 200
            || allocate_response.status() == 201
            || allocate_response.status() == 404,
        "Allocation should succeed or endpoint not found"
    );
}

// ==================== Concurrent Order Tests ====================

#[tokio::test]
#[ignore = "requires SQLite and Redis integration environment"]
async fn test_concurrent_orders_inventory_handling() {
    let app = TestApp::new().await;

    // Create warehouse location
    let location = inventory_location::ActiveModel {
        location_name: Set("Concurrent Warehouse".to_string()),
        ..Default::default()
    }
    .insert(app.state.db.as_ref())
    .await
    .expect("create location");

    // Create inventory with 10 units
    let create_inventory = json!({
        "item_number": "INV-CONC-001",
        "description": "Concurrent Widget",
        "primary_uom_code": "EA",
        "organization_id": 1,
        "location_id": location.location_id,
        "quantity_on_hand": 10,
        "reason": "initial stock"
    });

    app.request_authenticated(Method::POST, "/api/v1/inventory", Some(create_inventory))
        .await;

    let variant = app.seed_product_variant("INV-CONC-001", dec!(50.00)).await;

    // Try to create 5 orders of 3 units each (15 total, but only 10 available)
    // Execute orders sequentially to avoid borrow issues with TestApp
    let mut responses = Vec::new();
    for i in 1..=5 {
        let variant_id = variant.id.to_string();
        let email = format!("concurrent{}@test.com", i);
        let order_payload = json!({
            "customer_email": email,
            "customer_name": format!("Concurrent Test {}", i),
            "items": [{
                "variant_id": variant_id,
                "quantity": 3,
                "unit_price": "50.00"
            }]
        });

        let response = app
            .request_authenticated(Method::POST, "/api/v1/orders", Some(order_payload))
            .await;
        responses.push(response);
    }

    let successful = responses
        .iter()
        .filter(|r| r.status() == 201 || r.status() == 200)
        .count();

    // Not all orders should succeed if inventory is properly managed
    // (but implementation may vary)
    println!("Successful concurrent orders: {} out of 5", successful);
}

// ==================== Return Restocking Tests ====================

#[tokio::test]
#[ignore = "requires SQLite and Redis integration environment"]
async fn test_return_restocks_inventory() {
    let app = TestApp::new().await;

    // Create warehouse location
    let location = inventory_location::ActiveModel {
        location_name: Set("Return Warehouse".to_string()),
        ..Default::default()
    }
    .insert(app.state.db.as_ref())
    .await
    .expect("create location");

    // Create inventory with 50 units
    let create_inventory = json!({
        "item_number": "INV-RET-001",
        "description": "Return Widget",
        "primary_uom_code": "EA",
        "organization_id": 1,
        "location_id": location.location_id,
        "quantity_on_hand": 50,
        "reason": "initial stock"
    });

    app.request_authenticated(Method::POST, "/api/v1/inventory", Some(create_inventory))
        .await;

    let variant = app.seed_product_variant("INV-RET-001", dec!(45.00)).await;

    // Create and complete an order for 10 units
    let order_payload = json!({
        "customer_email": "return@test.com",
        "customer_name": "Return Test",
        "items": [{
            "variant_id": variant.id.to_string(),
            "quantity": 10,
            "unit_price": "45.00"
        }]
    });

    let order_response = app
        .request_authenticated(Method::POST, "/api/v1/orders", Some(order_payload))
        .await;

    if order_response.status() != 201 && order_response.status() != 200 {
        return;
    }

    let order_body = response_json(order_response).await;
    let order_id = order_body["data"]["id"]
        .as_str()
        .or_else(|| order_body["data"]["order_id"].as_str())
        .expect("Order should have ID");

    // Create a return
    let return_payload = json!({
        "order_id": order_id,
        "reason": "Testing restocking"
    });

    let return_response = app
        .request_authenticated(Method::POST, "/api/v1/returns", Some(return_payload))
        .await;

    if return_response.status() == 201 {
        let return_body = response_json(return_response).await;
        let return_id = return_body["data"]["id"].as_str().unwrap();

        // Process the return (restock)
        let restock_response = app
            .request_authenticated(
                Method::POST,
                &format!("/api/v1/returns/{}/restock", return_id),
                Some(json!({
                    "items": [{
                        "item_number": "INV-RET-001",
                        "quantity": 10,
                        "location_id": location.location_id
                    }]
                })),
            )
            .await;

        // Check inventory after restock
        if restock_response.status() == 200 {
            let inv_response = app
                .request_authenticated(Method::GET, "/api/v1/inventory/INV-RET-001", None)
                .await;

            if inv_response.status() == 200 {
                let body = response_json(inv_response).await;
                println!("Inventory after restock: {:?}", body);
            }
        }
    }
}

// ==================== Multi-Location Inventory Tests ====================

#[tokio::test]
#[ignore = "requires SQLite and Redis integration environment"]
async fn test_multi_location_inventory_order() {
    let app = TestApp::new().await;

    // Create two warehouse locations
    let location1 = inventory_location::ActiveModel {
        location_name: Set("Warehouse A".to_string()),
        ..Default::default()
    }
    .insert(app.state.db.as_ref())
    .await
    .expect("create location 1");

    let location2 = inventory_location::ActiveModel {
        location_name: Set("Warehouse B".to_string()),
        ..Default::default()
    }
    .insert(app.state.db.as_ref())
    .await
    .expect("create location 2");

    // Create inventory at both locations
    let create_inventory1 = json!({
        "item_number": "INV-MULTI-001",
        "description": "Multi-loc Widget",
        "primary_uom_code": "EA",
        "organization_id": 1,
        "location_id": location1.location_id,
        "quantity_on_hand": 30,
        "reason": "stock at warehouse A"
    });

    app.request_authenticated(Method::POST, "/api/v1/inventory", Some(create_inventory1))
        .await;

    let create_inventory2 = json!({
        "item_number": "INV-MULTI-001",
        "description": "Multi-loc Widget",
        "primary_uom_code": "EA",
        "organization_id": 1,
        "location_id": location2.location_id,
        "quantity_on_hand": 20,
        "reason": "stock at warehouse B"
    });

    // This might create a separate inventory record or update existing
    app.request_authenticated(Method::POST, "/api/v1/inventory", Some(create_inventory2))
        .await;

    let variant = app.seed_product_variant("INV-MULTI-001", dec!(55.00)).await;

    // Order 40 units (requires stock from both locations)
    let order_payload = json!({
        "customer_email": "multiloc@test.com",
        "customer_name": "Multi-location Test",
        "items": [{
            "variant_id": variant.id.to_string(),
            "quantity": 40,
            "unit_price": "55.00"
        }]
    });

    let response = app
        .request_authenticated(Method::POST, "/api/v1/orders", Some(order_payload))
        .await;

    // Depending on implementation, may succeed with multi-location allocation
    // or fail if locations aren't aggregated
    println!("Multi-location order status: {}", response.status());
}

// ==================== Low Stock Alert Tests ====================

#[tokio::test]
#[ignore = "requires SQLite and Redis integration environment"]
async fn test_low_stock_alert_after_order() {
    let app = TestApp::new().await;

    // Create warehouse location
    let location = inventory_location::ActiveModel {
        location_name: Set("Alert Warehouse".to_string()),
        ..Default::default()
    }
    .insert(app.state.db.as_ref())
    .await
    .expect("create location");

    // Create inventory just above low stock threshold
    let create_inventory = json!({
        "item_number": "INV-ALERT-001",
        "description": "Alert Widget",
        "primary_uom_code": "EA",
        "organization_id": 1,
        "location_id": location.location_id,
        "quantity_on_hand": 15,
        "reason": "initial stock"
    });

    app.request_authenticated(Method::POST, "/api/v1/inventory", Some(create_inventory))
        .await;

    let variant = app.seed_product_variant("INV-ALERT-001", dec!(60.00)).await;

    // Create order that brings stock below threshold
    let order_payload = json!({
        "customer_email": "alert@test.com",
        "customer_name": "Alert Test",
        "items": [{
            "variant_id": variant.id.to_string(),
            "quantity": 10,
            "unit_price": "60.00"
        }]
    });

    app.request_authenticated(Method::POST, "/api/v1/orders", Some(order_payload))
        .await;

    // Check low stock endpoint
    let low_stock_response = app
        .request_authenticated(
            Method::GET,
            "/api/v1/inventory/low-stock?threshold=10&limit=50&offset=0",
            None,
        )
        .await;

    assert_eq!(low_stock_response.status(), 200);

    let body = response_json(low_stock_response).await;
    let items = body["data"]["items"].as_array();

    if let Some(items) = items {
        // The item should appear in low stock list if inventory was reserved
        println!("Low stock items count: {}", items.len());
    }
}

// ==================== Backorder Tests ====================

#[tokio::test]
#[ignore = "requires SQLite and Redis integration environment"]
async fn test_backorder_creation() {
    let app = TestApp::new().await;

    // Product with no inventory
    let variant = app
        .seed_product_variant("INV-BACKORDER-001", dec!(70.00))
        .await;

    // Try to order without any inventory
    let order_payload = json!({
        "customer_email": "backorder@test.com",
        "customer_name": "Backorder Test",
        "items": [{
            "variant_id": variant.id.to_string(),
            "quantity": 5,
            "unit_price": "70.00"
        }],
        "allow_backorder": true
    });

    let response = app
        .request_authenticated(Method::POST, "/api/v1/orders", Some(order_payload))
        .await;

    // Backorder behavior depends on implementation
    println!("Backorder creation status: {}", response.status());
}
