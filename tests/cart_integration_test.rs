//! Comprehensive integration tests for the Cart service.
//!
//! Tests cover:
//! - Cart creation and lifecycle
//! - Adding/updating/removing items
//! - Cart abandonment and clearing
//! - Cart → Checkout flow integration
//! - Cart expiration handling
//! - Edge cases and validation

mod common;

use axum::{body, http::Method, response::Response};
use common::TestApp;
use rust_decimal_macros::dec;
use serde_json::{json, Value};

async fn response_json(response: Response) -> Value {
    let bytes = body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("response body bytes");
    serde_json::from_slice(&bytes).expect("json response")
}

// ==================== Cart Creation Tests ====================

#[tokio::test]
#[ignore = "requires SQLite and Redis integration environment"]
async fn test_create_cart_success() {
    let app = TestApp::new().await;

    let payload = json!({
        "currency": "USD",
        "session_id": "session_123"
    });

    let response = app
        .request_authenticated(Method::POST, "/api/v1/carts", Some(payload))
        .await;

    assert_eq!(response.status(), 201, "Cart creation should succeed");

    let body = response_json(response).await;
    assert!(body["success"].as_bool().unwrap_or(false));

    let cart = &body["data"];
    assert!(cart["id"].as_str().is_some(), "Cart should have an ID");
    assert_eq!(cart["currency"], "USD");
    assert_eq!(cart["status"], "Active");
    assert_eq!(cart["subtotal"], "0");
    assert_eq!(cart["total"], "0");
}

#[tokio::test]
#[ignore = "requires SQLite and Redis integration environment"]
async fn test_create_cart_default_currency() {
    let app = TestApp::new().await;

    let payload = json!({});

    let response = app
        .request_authenticated(Method::POST, "/api/v1/carts", Some(payload))
        .await;

    assert_eq!(response.status(), 201);

    let body = response_json(response).await;
    let cart = &body["data"];
    assert_eq!(cart["currency"], "USD", "Default currency should be USD");
}

#[tokio::test]
#[ignore = "requires SQLite and Redis integration environment"]
async fn test_create_cart_with_metadata() {
    let app = TestApp::new().await;

    let payload = json!({
        "currency": "EUR",
        "metadata": {
            "source": "mobile_app",
            "campaign": "summer_sale"
        }
    });

    let response = app
        .request_authenticated(Method::POST, "/api/v1/carts", Some(payload))
        .await;

    assert_eq!(response.status(), 201);
    let body = response_json(response).await;
    assert!(body["success"].as_bool().unwrap_or(false));
}

// ==================== Cart Retrieval Tests ====================

#[tokio::test]
#[ignore = "requires SQLite and Redis integration environment"]
async fn test_get_cart_by_id() {
    let app = TestApp::new().await;

    // Create a cart first
    let create_response = app
        .request_authenticated(Method::POST, "/api/v1/carts", Some(json!({})))
        .await;
    assert_eq!(create_response.status(), 201);

    let create_body = response_json(create_response).await;
    let cart_id = create_body["data"]["id"].as_str().unwrap();

    // Retrieve the cart
    let get_response = app
        .request_authenticated(Method::GET, &format!("/api/v1/carts/{}", cart_id), None)
        .await;

    assert_eq!(get_response.status(), 200);
    let body = response_json(get_response).await;
    assert_eq!(body["data"]["cart"]["id"], cart_id);
}

#[tokio::test]
#[ignore = "requires SQLite and Redis integration environment"]
async fn test_get_nonexistent_cart_returns_404() {
    let app = TestApp::new().await;

    let fake_id = "00000000-0000-0000-0000-000000000000";
    let response = app
        .request_authenticated(Method::GET, &format!("/api/v1/carts/{}", fake_id), None)
        .await;

    assert_eq!(response.status(), 404);
}

#[tokio::test]
#[ignore = "requires SQLite and Redis integration environment"]
async fn test_list_carts_pagination() {
    let app = TestApp::new().await;

    // Create multiple carts
    for _ in 0..5 {
        app.request_authenticated(Method::POST, "/api/v1/carts", Some(json!({})))
            .await;
    }

    // List with pagination
    let response = app
        .request_authenticated(Method::GET, "/api/v1/carts?page=1&per_page=3", None)
        .await;

    assert_eq!(response.status(), 200);
    let body = response_json(response).await;
    let items = body["data"]["items"].as_array().unwrap();
    assert!(items.len() <= 3);
}

// ==================== Cart Item Management Tests ====================

#[tokio::test]
#[ignore = "requires SQLite and Redis integration environment"]
async fn test_add_item_to_cart() {
    let app = TestApp::new().await;

    // Create cart
    let cart_response = app
        .request_authenticated(Method::POST, "/api/v1/carts", Some(json!({})))
        .await;
    let cart_body = response_json(cart_response).await;
    let cart_id = cart_body["data"]["id"].as_str().unwrap();

    // Seed a product variant
    let variant = app.seed_product_variant("CART-TEST-SKU", dec!(29.99)).await;

    // Add item to cart
    let add_payload = json!({
        "variant_id": variant.id.to_string(),
        "quantity": 2
    });

    let response = app
        .request_authenticated(
            Method::POST,
            &format!("/api/v1/carts/{}/items", cart_id),
            Some(add_payload),
        )
        .await;

    assert_eq!(response.status(), 200);
    let body = response_json(response).await;

    // Verify totals are updated
    let cart = &body["data"];
    assert!(cart["subtotal"].as_str().unwrap().parse::<f64>().unwrap() > 0.0);
}

#[tokio::test]
#[ignore = "requires SQLite and Redis integration environment"]
async fn test_add_same_item_increases_quantity() {
    let app = TestApp::new().await;

    // Create cart
    let cart_response = app
        .request_authenticated(Method::POST, "/api/v1/carts", Some(json!({})))
        .await;
    let cart_body = response_json(cart_response).await;
    let cart_id = cart_body["data"]["id"].as_str().unwrap();

    // Seed a product variant
    let variant = app.seed_product_variant("CART-DUP-SKU", dec!(10.00)).await;

    // Add item twice
    let add_payload = json!({
        "variant_id": variant.id.to_string(),
        "quantity": 1
    });

    app.request_authenticated(
        Method::POST,
        &format!("/api/v1/carts/{}/items", cart_id),
        Some(add_payload.clone()),
    )
    .await;

    let response = app
        .request_authenticated(
            Method::POST,
            &format!("/api/v1/carts/{}/items", cart_id),
            Some(add_payload),
        )
        .await;

    assert_eq!(response.status(), 200);

    // Get cart to verify quantity
    let get_response = app
        .request_authenticated(Method::GET, &format!("/api/v1/carts/{}", cart_id), None)
        .await;
    let body = response_json(get_response).await;
    let items = body["data"]["items"].as_array().unwrap();

    assert_eq!(items.len(), 1, "Should have one item with increased quantity");
    assert_eq!(items[0]["quantity"], 2);
}

#[tokio::test]
#[ignore = "requires SQLite and Redis integration environment"]
async fn test_add_item_invalid_quantity_fails() {
    let app = TestApp::new().await;

    // Create cart
    let cart_response = app
        .request_authenticated(Method::POST, "/api/v1/carts", Some(json!({})))
        .await;
    let cart_body = response_json(cart_response).await;
    let cart_id = cart_body["data"]["id"].as_str().unwrap();

    // Seed a product variant
    let variant = app.seed_product_variant("CART-INV-SKU", dec!(10.00)).await;

    // Try to add with invalid quantity
    let add_payload = json!({
        "variant_id": variant.id.to_string(),
        "quantity": 0
    });

    let response = app
        .request_authenticated(
            Method::POST,
            &format!("/api/v1/carts/{}/items", cart_id),
            Some(add_payload),
        )
        .await;

    assert_eq!(response.status(), 400, "Zero quantity should be rejected");
}

#[tokio::test]
#[ignore = "requires SQLite and Redis integration environment"]
async fn test_update_cart_item_quantity() {
    let app = TestApp::new().await;

    // Create cart and add item
    let cart_response = app
        .request_authenticated(Method::POST, "/api/v1/carts", Some(json!({})))
        .await;
    let cart_body = response_json(cart_response).await;
    let cart_id = cart_body["data"]["id"].as_str().unwrap();

    let variant = app.seed_product_variant("CART-UPD-SKU", dec!(15.00)).await;

    let add_payload = json!({
        "variant_id": variant.id.to_string(),
        "quantity": 1
    });

    app.request_authenticated(
        Method::POST,
        &format!("/api/v1/carts/{}/items", cart_id),
        Some(add_payload),
    )
    .await;

    // Get cart to find item ID
    let get_response = app
        .request_authenticated(Method::GET, &format!("/api/v1/carts/{}", cart_id), None)
        .await;
    let body = response_json(get_response).await;
    let item_id = body["data"]["items"][0]["id"].as_str().unwrap();

    // Update quantity
    let update_payload = json!({ "quantity": 5 });
    let response = app
        .request_authenticated(
            Method::PUT,
            &format!("/api/v1/carts/{}/items/{}", cart_id, item_id),
            Some(update_payload),
        )
        .await;

    assert_eq!(response.status(), 200);

    // Verify updated quantity
    let get_response = app
        .request_authenticated(Method::GET, &format!("/api/v1/carts/{}", cart_id), None)
        .await;
    let body = response_json(get_response).await;
    assert_eq!(body["data"]["items"][0]["quantity"], 5);
}

#[tokio::test]
#[ignore = "requires SQLite and Redis integration environment"]
async fn test_remove_cart_item() {
    let app = TestApp::new().await;

    // Create cart and add item
    let cart_response = app
        .request_authenticated(Method::POST, "/api/v1/carts", Some(json!({})))
        .await;
    let cart_body = response_json(cart_response).await;
    let cart_id = cart_body["data"]["id"].as_str().unwrap();

    let variant = app.seed_product_variant("CART-REM-SKU", dec!(20.00)).await;

    let add_payload = json!({
        "variant_id": variant.id.to_string(),
        "quantity": 1
    });

    app.request_authenticated(
        Method::POST,
        &format!("/api/v1/carts/{}/items", cart_id),
        Some(add_payload),
    )
    .await;

    // Get cart to find item ID
    let get_response = app
        .request_authenticated(Method::GET, &format!("/api/v1/carts/{}", cart_id), None)
        .await;
    let body = response_json(get_response).await;
    let item_id = body["data"]["items"][0]["id"].as_str().unwrap();

    // Remove item
    let response = app
        .request_authenticated(
            Method::DELETE,
            &format!("/api/v1/carts/{}/items/{}", cart_id, item_id),
            None,
        )
        .await;

    assert_eq!(response.status(), 204);

    // Verify item is removed
    let get_response = app
        .request_authenticated(Method::GET, &format!("/api/v1/carts/{}", cart_id), None)
        .await;
    let body = response_json(get_response).await;
    let items = body["data"]["items"].as_array().unwrap();
    assert!(items.is_empty(), "Cart should be empty after removing item");
}

// ==================== Cart Clear Tests ====================

#[tokio::test]
#[ignore = "requires SQLite and Redis integration environment"]
async fn test_clear_cart() {
    let app = TestApp::new().await;

    // Create cart and add multiple items
    let cart_response = app
        .request_authenticated(Method::POST, "/api/v1/carts", Some(json!({})))
        .await;
    let cart_body = response_json(cart_response).await;
    let cart_id = cart_body["data"]["id"].as_str().unwrap();

    let variant1 = app.seed_product_variant("CART-CLR-1", dec!(10.00)).await;
    let variant2 = app.seed_product_variant("CART-CLR-2", dec!(20.00)).await;

    app.request_authenticated(
        Method::POST,
        &format!("/api/v1/carts/{}/items", cart_id),
        Some(json!({ "variant_id": variant1.id.to_string(), "quantity": 1 })),
    )
    .await;

    app.request_authenticated(
        Method::POST,
        &format!("/api/v1/carts/{}/items", cart_id),
        Some(json!({ "variant_id": variant2.id.to_string(), "quantity": 2 })),
    )
    .await;

    // Clear cart
    let response = app
        .request_authenticated(
            Method::POST,
            &format!("/api/v1/carts/{}/clear", cart_id),
            None,
        )
        .await;

    assert_eq!(response.status(), 200);

    // Verify cart is empty
    let get_response = app
        .request_authenticated(Method::GET, &format!("/api/v1/carts/{}", cart_id), None)
        .await;
    let body = response_json(get_response).await;
    let items = body["data"]["items"].as_array().unwrap();
    assert!(items.is_empty(), "Cart should be empty after clear");
}

// ==================== Cart Abandonment Tests ====================

#[tokio::test]
#[ignore = "requires SQLite and Redis integration environment"]
async fn test_abandon_cart() {
    let app = TestApp::new().await;

    // Create cart
    let cart_response = app
        .request_authenticated(Method::POST, "/api/v1/carts", Some(json!({})))
        .await;
    let cart_body = response_json(cart_response).await;
    let cart_id = cart_body["data"]["id"].as_str().unwrap();

    // Abandon cart (DELETE)
    let response = app
        .request_authenticated(Method::DELETE, &format!("/api/v1/carts/{}", cart_id), None)
        .await;

    assert_eq!(response.status(), 200);

    let body = response_json(response).await;
    assert_eq!(body["data"]["status"], "Abandoned");
}

// ==================== Cart Total Calculation Tests ====================

#[tokio::test]
#[ignore = "requires SQLite and Redis integration environment"]
async fn test_cart_total_calculation() {
    let app = TestApp::new().await;

    // Create cart
    let cart_response = app
        .request_authenticated(Method::POST, "/api/v1/carts", Some(json!({})))
        .await;
    let cart_body = response_json(cart_response).await;
    let cart_id = cart_body["data"]["id"].as_str().unwrap();

    // Add items with known prices
    let variant1 = app.seed_product_variant("CART-CALC-1", dec!(25.00)).await;
    let variant2 = app.seed_product_variant("CART-CALC-2", dec!(15.50)).await;

    // Add 2 of variant1 = $50.00
    app.request_authenticated(
        Method::POST,
        &format!("/api/v1/carts/{}/items", cart_id),
        Some(json!({ "variant_id": variant1.id.to_string(), "quantity": 2 })),
    )
    .await;

    // Add 3 of variant2 = $46.50
    let response = app
        .request_authenticated(
            Method::POST,
            &format!("/api/v1/carts/{}/items", cart_id),
            Some(json!({ "variant_id": variant2.id.to_string(), "quantity": 3 })),
        )
        .await;

    let body = response_json(response).await;
    let subtotal: f64 = body["data"]["subtotal"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap_or(0.0);

    // Expected: $50.00 + $46.50 = $96.50
    assert!(
        (subtotal - 96.50).abs() < 0.01,
        "Subtotal should be $96.50, got {}",
        subtotal
    );
}

// ==================== Edge Case Tests ====================

#[tokio::test]
#[ignore = "requires SQLite and Redis integration environment"]
async fn test_add_item_to_nonexistent_cart_fails() {
    let app = TestApp::new().await;

    let variant = app.seed_product_variant("CART-NOEXIST", dec!(10.00)).await;
    let fake_cart_id = "00000000-0000-0000-0000-000000000000";

    let response = app
        .request_authenticated(
            Method::POST,
            &format!("/api/v1/carts/{}/items", fake_cart_id),
            Some(json!({ "variant_id": variant.id.to_string(), "quantity": 1 })),
        )
        .await;

    assert!(
        response.status() == 404 || response.status() == 401,
        "Should fail for nonexistent cart"
    );
}

#[tokio::test]
#[ignore = "requires SQLite and Redis integration environment"]
async fn test_add_nonexistent_variant_fails() {
    let app = TestApp::new().await;

    // Create cart
    let cart_response = app
        .request_authenticated(Method::POST, "/api/v1/carts", Some(json!({})))
        .await;
    let cart_body = response_json(cart_response).await;
    let cart_id = cart_body["data"]["id"].as_str().unwrap();

    let fake_variant_id = "00000000-0000-0000-0000-000000000000";

    let response = app
        .request_authenticated(
            Method::POST,
            &format!("/api/v1/carts/{}/items", cart_id),
            Some(json!({ "variant_id": fake_variant_id, "quantity": 1 })),
        )
        .await;

    assert_eq!(response.status(), 404, "Should fail for nonexistent variant");
}

#[tokio::test]
#[ignore = "requires SQLite and Redis integration environment"]
async fn test_cart_requires_authentication() {
    let app = TestApp::new().await;

    // Try to create cart without auth
    let response = app
        .request(Method::POST, "/api/v1/carts", Some(json!({})), None)
        .await;

    assert_eq!(response.status(), 401, "Unauthenticated request should fail");
}

// ==================== Multiple Items Tests ====================

#[tokio::test]
#[ignore = "requires SQLite and Redis integration environment"]
async fn test_cart_with_multiple_different_items() {
    let app = TestApp::new().await;

    // Create cart
    let cart_response = app
        .request_authenticated(Method::POST, "/api/v1/carts", Some(json!({})))
        .await;
    let cart_body = response_json(cart_response).await;
    let cart_id = cart_body["data"]["id"].as_str().unwrap();

    // Add multiple different items
    let variants: Vec<_> = futures::future::join_all((1..=5).map(|i| {
        let app = &app;
        async move {
            app.seed_product_variant(&format!("MULTI-{}", i), dec!(10.00) * rust_decimal::Decimal::from(i))
                .await
        }
    }))
    .await;

    for variant in &variants {
        app.request_authenticated(
            Method::POST,
            &format!("/api/v1/carts/{}/items", cart_id),
            Some(json!({ "variant_id": variant.id.to_string(), "quantity": 1 })),
        )
        .await;
    }

    // Verify all items are in cart
    let get_response = app
        .request_authenticated(Method::GET, &format!("/api/v1/carts/{}", cart_id), None)
        .await;
    let body = response_json(get_response).await;
    let items = body["data"]["items"].as_array().unwrap();

    assert_eq!(items.len(), 5, "Cart should have 5 different items");
}
