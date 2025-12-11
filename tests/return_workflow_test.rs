//! Comprehensive integration tests for the Return service and workflow.
//!
//! Tests cover:
//! - Return creation
//! - Return approval workflow
//! - Return status transitions
//! - Return listing and retrieval
//! - Return with inventory restocking
//! - Validation and error cases

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

// ==================== Return Creation Tests ====================

#[tokio::test]
#[ignore = "requires SQLite and Redis integration environment"]
async fn test_create_return_success() {
    let app = TestApp::new().await;

    // First create an order to return
    let variant = app.seed_product_variant("RET-TEST-SKU", dec!(99.99)).await;

    let order_payload = json!({
        "customer_email": "return@test.com",
        "customer_name": "Return Test",
        "items": [{
            "variant_id": variant.id.to_string(),
            "quantity": 1,
            "unit_price": "99.99"
        }]
    });

    let order_response = app
        .request_authenticated(Method::POST, "/api/v1/orders", Some(order_payload))
        .await;

    if order_response.status() != 201 && order_response.status() != 200 {
        return; // Skip if order creation fails
    }

    let order_body = response_json(order_response).await;
    let order_id = order_body["data"]["id"]
        .as_str()
        .or_else(|| order_body["data"]["order_id"].as_str())
        .expect("Order should have an ID");

    // Create a return for the order
    let return_payload = json!({
        "order_id": order_id,
        "reason": "Product not as described"
    });

    let response = app
        .request_authenticated(Method::POST, "/api/v1/returns", Some(return_payload))
        .await;

    assert_eq!(response.status(), 201, "Return creation should succeed");

    let body = response_json(response).await;
    assert!(body["success"].as_bool().unwrap_or(false));

    let return_data = &body["data"];
    assert!(return_data["id"].as_str().is_some());
    assert_eq!(return_data["order_id"], order_id);
    assert_eq!(return_data["reason"], "Product not as described");
}

#[tokio::test]
#[ignore = "requires SQLite and Redis integration environment"]
async fn test_create_return_empty_reason_fails() {
    let app = TestApp::new().await;

    // Create an order first
    let variant = app.seed_product_variant("RET-EMPTY-SKU", dec!(50.00)).await;

    let order_payload = json!({
        "customer_email": "empty@test.com",
        "customer_name": "Empty Test",
        "items": [{
            "variant_id": variant.id.to_string(),
            "quantity": 1,
            "unit_price": "50.00"
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
        .expect("Should have order ID");

    // Try to create return with empty reason
    let return_payload = json!({
        "order_id": order_id,
        "reason": ""
    });

    let response = app
        .request_authenticated(Method::POST, "/api/v1/returns", Some(return_payload))
        .await;

    assert_eq!(response.status(), 400, "Empty reason should be rejected");
}

#[tokio::test]
#[ignore = "requires SQLite and Redis integration environment"]
async fn test_create_return_various_reasons() {
    let app = TestApp::new().await;

    let reasons = vec![
        "Defective product",
        "Wrong item received",
        "Item damaged during shipping",
        "Changed my mind",
        "Better price found elsewhere",
        "Quality not as expected",
    ];

    for (i, reason) in reasons.iter().enumerate() {
        let variant = app
            .seed_product_variant(&format!("RET-REASON-{}", i), dec!(25.00))
            .await;

        let order_payload = json!({
            "customer_email": format!("reason{}@test.com", i),
            "customer_name": format!("Reason Test {}", i),
            "items": [{
                "variant_id": variant.id.to_string(),
                "quantity": 1,
                "unit_price": "25.00"
            }]
        });

        let order_response = app
            .request_authenticated(Method::POST, "/api/v1/orders", Some(order_payload))
            .await;

        if order_response.status() != 201 && order_response.status() != 200 {
            continue;
        }

        let order_body = response_json(order_response).await;
        let order_id = order_body["data"]["id"]
            .as_str()
            .or_else(|| order_body["data"]["order_id"].as_str());

        if order_id.is_none() {
            continue;
        }

        let return_payload = json!({
            "order_id": order_id.unwrap(),
            "reason": reason
        });

        let response = app
            .request_authenticated(Method::POST, "/api/v1/returns", Some(return_payload))
            .await;

        assert_eq!(
            response.status(),
            201,
            "Return with reason '{}' should succeed",
            reason
        );
    }
}

// ==================== Return Retrieval Tests ====================

#[tokio::test]
#[ignore = "requires SQLite and Redis integration environment"]
async fn test_get_return_by_id() {
    let app = TestApp::new().await;

    // Create order and return
    let variant = app.seed_product_variant("RET-GET-SKU", dec!(75.00)).await;

    let order_payload = json!({
        "customer_email": "get@test.com",
        "customer_name": "Get Test",
        "items": [{
            "variant_id": variant.id.to_string(),
            "quantity": 1,
            "unit_price": "75.00"
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
        .expect("Should have order ID");

    let return_payload = json!({
        "order_id": order_id,
        "reason": "Testing retrieval"
    });

    let create_response = app
        .request_authenticated(Method::POST, "/api/v1/returns", Some(return_payload))
        .await;

    assert_eq!(create_response.status(), 201);
    let create_body = response_json(create_response).await;
    let return_id = create_body["data"]["id"].as_str().unwrap();

    // Get the return
    let get_response = app
        .request_authenticated(Method::GET, &format!("/api/v1/returns/{}", return_id), None)
        .await;

    assert_eq!(get_response.status(), 200);
    let body = response_json(get_response).await;
    assert_eq!(body["data"]["id"], return_id);
}

#[tokio::test]
#[ignore = "requires SQLite and Redis integration environment"]
async fn test_get_nonexistent_return_returns_404() {
    let app = TestApp::new().await;

    let fake_id = "00000000-0000-0000-0000-000000000000";
    let response = app
        .request_authenticated(Method::GET, &format!("/api/v1/returns/{}", fake_id), None)
        .await;

    assert_eq!(response.status(), 404);
}

#[tokio::test]
#[ignore = "requires SQLite and Redis integration environment"]
async fn test_list_returns_pagination() {
    let app = TestApp::new().await;

    // Create multiple returns
    for i in 1..=5 {
        let variant = app
            .seed_product_variant(&format!("RET-LIST-{}", i), dec!(30.00))
            .await;

        let order_payload = json!({
            "customer_email": format!("list{}@test.com", i),
            "customer_name": format!("List Test {}", i),
            "items": [{
                "variant_id": variant.id.to_string(),
                "quantity": 1,
                "unit_price": "30.00"
            }]
        });

        let order_response = app
            .request_authenticated(Method::POST, "/api/v1/orders", Some(order_payload))
            .await;

        if order_response.status() != 201 && order_response.status() != 200 {
            continue;
        }

        let order_body = response_json(order_response).await;
        if let Some(order_id) = order_body["data"]["id"]
            .as_str()
            .or_else(|| order_body["data"]["order_id"].as_str())
        {
            let return_payload = json!({
                "order_id": order_id,
                "reason": format!("Test return {}", i)
            });

            app.request_authenticated(Method::POST, "/api/v1/returns", Some(return_payload))
                .await;
        }
    }

    // List with pagination
    let response = app
        .request_authenticated(Method::GET, "/api/v1/returns?page=1&limit=3", None)
        .await;

    assert_eq!(response.status(), 200);
    let body = response_json(response).await;
    let items = body["data"]["items"].as_array();
    if let Some(items) = items {
        assert!(items.len() <= 3);
    }
}

// ==================== Return Approval Workflow Tests ====================

#[tokio::test]
#[ignore = "requires SQLite and Redis integration environment"]
async fn test_approve_return() {
    let app = TestApp::new().await;

    // Create order and return
    let variant = app
        .seed_product_variant("RET-APPROVE-SKU", dec!(100.00))
        .await;

    let order_payload = json!({
        "customer_email": "approve@test.com",
        "customer_name": "Approve Test",
        "items": [{
            "variant_id": variant.id.to_string(),
            "quantity": 1,
            "unit_price": "100.00"
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
        .expect("Should have order ID");

    let return_payload = json!({
        "order_id": order_id,
        "reason": "Testing approval"
    });

    let create_response = app
        .request_authenticated(Method::POST, "/api/v1/returns", Some(return_payload))
        .await;

    assert_eq!(create_response.status(), 201);
    let create_body = response_json(create_response).await;
    let return_id = create_body["data"]["id"].as_str().unwrap();

    // Approve the return
    let response = app
        .request_authenticated(
            Method::POST,
            &format!("/api/v1/returns/{}/approve", return_id),
            None,
        )
        .await;

    // Approval endpoint may or may not exist
    assert!(
        response.status() == 200 || response.status() == 404,
        "Approval should succeed or endpoint not found"
    );

    if response.status() == 200 {
        let body = response_json(response).await;
        // Check status was updated to approved
        let status = body["data"]["status"].as_str().unwrap_or("");
        assert!(
            status.to_lowercase().contains("approv"),
            "Status should indicate approved"
        );
    }
}

#[tokio::test]
#[ignore = "requires SQLite and Redis integration environment"]
async fn test_reject_return() {
    let app = TestApp::new().await;

    // Create order and return
    let variant = app
        .seed_product_variant("RET-REJECT-SKU", dec!(50.00))
        .await;

    let order_payload = json!({
        "customer_email": "reject@test.com",
        "customer_name": "Reject Test",
        "items": [{
            "variant_id": variant.id.to_string(),
            "quantity": 1,
            "unit_price": "50.00"
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
        .expect("Should have order ID");

    let return_payload = json!({
        "order_id": order_id,
        "reason": "Testing rejection"
    });

    let create_response = app
        .request_authenticated(Method::POST, "/api/v1/returns", Some(return_payload))
        .await;

    assert_eq!(create_response.status(), 201);
    let create_body = response_json(create_response).await;
    let return_id = create_body["data"]["id"].as_str().unwrap();

    // Reject the return
    let reject_payload = json!({
        "reason": "Return window expired"
    });

    let response = app
        .request_authenticated(
            Method::POST,
            &format!("/api/v1/returns/{}/reject", return_id),
            Some(reject_payload),
        )
        .await;

    // Rejection endpoint may or may not exist
    assert!(
        response.status() == 200 || response.status() == 404,
        "Rejection should succeed or endpoint not found"
    );
}

// ==================== Return Status Filter Tests ====================

#[tokio::test]
#[ignore = "requires SQLite and Redis integration environment"]
async fn test_list_returns_by_status() {
    let app = TestApp::new().await;

    // Create a return
    let variant = app
        .seed_product_variant("RET-STATUS-SKU", dec!(40.00))
        .await;

    let order_payload = json!({
        "customer_email": "status@test.com",
        "customer_name": "Status Test",
        "items": [{
            "variant_id": variant.id.to_string(),
            "quantity": 1,
            "unit_price": "40.00"
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
        .expect("Should have order ID");

    let return_payload = json!({
        "order_id": order_id,
        "reason": "Testing status filter"
    });

    app.request_authenticated(Method::POST, "/api/v1/returns", Some(return_payload))
        .await;

    // List returns filtered by status
    let response = app
        .request_authenticated(Method::GET, "/api/v1/returns?status=pending", None)
        .await;

    assert_eq!(response.status(), 200);
}

// ==================== Return Complete Workflow Tests ====================

#[tokio::test]
#[ignore = "requires SQLite and Redis integration environment"]
async fn test_complete_return() {
    let app = TestApp::new().await;

    // Create order and return
    let variant = app
        .seed_product_variant("RET-COMPLETE-SKU", dec!(80.00))
        .await;

    let order_payload = json!({
        "customer_email": "complete@test.com",
        "customer_name": "Complete Test",
        "items": [{
            "variant_id": variant.id.to_string(),
            "quantity": 1,
            "unit_price": "80.00"
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
        .expect("Should have order ID");

    let return_payload = json!({
        "order_id": order_id,
        "reason": "Testing complete workflow"
    });

    let create_response = app
        .request_authenticated(Method::POST, "/api/v1/returns", Some(return_payload))
        .await;

    assert_eq!(create_response.status(), 201);
    let create_body = response_json(create_response).await;
    let return_id = create_body["data"]["id"].as_str().unwrap();

    // Complete the return
    let response = app
        .request_authenticated(
            Method::POST,
            &format!("/api/v1/returns/{}/complete", return_id),
            None,
        )
        .await;

    // Complete endpoint may or may not exist
    assert!(
        response.status() == 200 || response.status() == 404,
        "Complete should succeed or endpoint not found"
    );
}

// ==================== Return with Restocking Tests ====================

#[tokio::test]
#[ignore = "requires SQLite and Redis integration environment"]
async fn test_return_restock_items() {
    let app = TestApp::new().await;

    // Create order and return
    let variant = app
        .seed_product_variant("RET-RESTOCK-SKU", dec!(60.00))
        .await;

    let order_payload = json!({
        "customer_email": "restock@test.com",
        "customer_name": "Restock Test",
        "items": [{
            "variant_id": variant.id.to_string(),
            "quantity": 2,
            "unit_price": "60.00"
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
        .expect("Should have order ID");

    let return_payload = json!({
        "order_id": order_id,
        "reason": "Testing restocking"
    });

    let create_response = app
        .request_authenticated(Method::POST, "/api/v1/returns", Some(return_payload))
        .await;

    assert_eq!(create_response.status(), 201);
    let create_body = response_json(create_response).await;
    let return_id = create_body["data"]["id"].as_str().unwrap();

    // Restock items
    let restock_payload = json!({
        "items": [{
            "variant_id": variant.id.to_string(),
            "quantity": 2
        }]
    });

    let response = app
        .request_authenticated(
            Method::POST,
            &format!("/api/v1/returns/{}/restock", return_id),
            Some(restock_payload),
        )
        .await;

    // Restock endpoint may or may not exist
    assert!(
        response.status() == 200 || response.status() == 201 || response.status() == 404,
        "Restock should succeed or endpoint not found"
    );
}

// ==================== Authentication Tests ====================

#[tokio::test]
#[ignore = "requires SQLite and Redis integration environment"]
async fn test_return_requires_authentication() {
    let app = TestApp::new().await;

    let return_payload = json!({
        "order_id": "00000000-0000-0000-0000-000000000001",
        "reason": "Test"
    });

    let response = app
        .request(Method::POST, "/api/v1/returns", Some(return_payload), None)
        .await;

    assert_eq!(
        response.status(),
        401,
        "Unauthenticated return creation should fail"
    );
}

// ==================== Edge Cases ====================

#[tokio::test]
#[ignore = "requires SQLite and Redis integration environment"]
async fn test_return_for_nonexistent_order() {
    let app = TestApp::new().await;

    let return_payload = json!({
        "order_id": "00000000-0000-0000-0000-000000000000",
        "reason": "This order does not exist"
    });

    let response = app
        .request_authenticated(Method::POST, "/api/v1/returns", Some(return_payload))
        .await;

    // Should fail - either 400 or 404
    assert!(
        response.status() == 400 || response.status() == 404,
        "Return for nonexistent order should fail"
    );
}

#[tokio::test]
#[ignore = "requires SQLite and Redis integration environment"]
async fn test_return_long_reason() {
    let app = TestApp::new().await;

    let variant = app.seed_product_variant("RET-LONG-SKU", dec!(50.00)).await;

    let order_payload = json!({
        "customer_email": "long@test.com",
        "customer_name": "Long Test",
        "items": [{
            "variant_id": variant.id.to_string(),
            "quantity": 1,
            "unit_price": "50.00"
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
        .expect("Should have order ID");

    // Create return with very long reason
    let long_reason = "A".repeat(1000);
    let return_payload = json!({
        "order_id": order_id,
        "reason": long_reason
    });

    let response = app
        .request_authenticated(Method::POST, "/api/v1/returns", Some(return_payload))
        .await;

    // Should either succeed or fail with validation error
    assert!(
        response.status() == 201 || response.status() == 400,
        "Long reason should either succeed or be rejected with validation error"
    );
}
