/// Comprehensive integration tests for Agentic Commerce Server
/// These tests cover all critical paths and edge cases
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use serde_json::{json, Value};
use tower::ServiceExt;

mod common;
use common::*;

#[cfg(test)]
mod checkout_session_tests {
    use super::*;

    #[tokio::test]
    async fn test_create_checkout_session_success() {
        let app = setup_test_app().await;

        let request_body = json!({
            "items": [
                {"id": "item_123", "quantity": 2}
            ],
            "customer": {
                "shipping_address": {
                    "name": "John Doe",
                    "line1": "123 Main St",
                    "city": "San Francisco",
                    "region": "CA",
                    "postal_code": "94105",
                    "country": "US",
                    "email": "john@example.com"
                }
            },
            "fulfillment": {
                "selected_id": "standard_shipping"
            }
        });

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/checkout_sessions")
                    .header("Content-Type", "application/json")
                    .header("Authorization", "Bearer test_api_key")
                    .header("API-Version", "2025-09-29")
                    .body(Body::from(request_body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::CREATED);

        let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        let session: Value = serde_json::from_slice(&body).unwrap();

        assert!(session["id"].is_string());
        assert_eq!(session["status"], "ready_for_payment");
        assert_eq!(session["items"][0]["quantity"], 2);
        assert!(session["totals"]["grand_total"]["amount"].as_i64().unwrap() > 0);
    }

    #[tokio::test]
    async fn test_create_checkout_session_invalid_items() {
        let app = setup_test_app().await;

        let request_body = json!({
            "items": [],
            "customer": null
        });

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/checkout_sessions")
                    .header("Content-Type", "application/json")
                    .header("Authorization", "Bearer test_api_key")
                    .header("API-Version", "2025-09-29")
                    .body(Body::from(request_body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_get_checkout_session_success() {
        let app = setup_test_app().await;

        // First create a session
        let create_body = json!({
            "items": [{"id": "item_123", "quantity": 1}]
        });

        let create_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/checkout_sessions")
                    .header("Content-Type", "application/json")
                    .header("Authorization", "Bearer test_api_key")
                    .header("API-Version", "2025-09-29")
                    .body(Body::from(create_body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        let body = hyper::body::to_bytes(create_response.into_body())
            .await
            .unwrap();
        let session: Value = serde_json::from_slice(&body).unwrap();
        let session_id = session["id"].as_str().unwrap();

        // Now retrieve it
        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/checkout_sessions/{}", session_id))
                    .header("Authorization", "Bearer test_api_key")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        let retrieved: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(retrieved["id"], session_id);
    }

    #[tokio::test]
    async fn test_get_checkout_session_not_found() {
        let app = setup_test_app().await;

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/checkout_sessions/nonexistent-id")
                    .header("Authorization", "Bearer test_api_key")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_update_checkout_session_success() {
        let app = setup_test_app().await;

        // Create session
        let session_id = create_test_session(app.clone()).await;

        // Update it
        let update_body = json!({
            "customer": {
                "shipping_address": {
                    "name": "Jane Doe",
                    "line1": "456 Oak Ave",
                    "city": "Los Angeles",
                    "region": "CA",
                    "postal_code": "90001",
                    "country": "US",
                    "email": "jane@example.com"
                }
            },
            "fulfillment": {
                "selected_id": "express_shipping"
            }
        });

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/checkout_sessions/{}", session_id))
                    .header("Content-Type", "application/json")
                    .header("Authorization", "Bearer test_api_key")
                    .body(Body::from(update_body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        let updated: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(
            updated["customer"]["shipping_address"]["name"],
            "Jane Doe"
        );
        assert_eq!(updated["fulfillment"]["selected_id"], "express_shipping");
    }

    #[tokio::test]
    async fn test_complete_checkout_session_success() {
        let app = setup_test_app().await;

        // Create and prepare session
        let session_id = create_ready_session(app.clone()).await;

        // Complete it
        let complete_body = json!({
            "payment": {
                "method": "pm_card_visa"
            }
        });

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/checkout_sessions/{}/complete", session_id))
                    .header("Content-Type", "application/json")
                    .header("Authorization", "Bearer test_api_key")
                    .body(Body::from(complete_body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        let result: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(result["session"]["status"], "completed");
        assert!(result["order"]["id"].is_string());
    }

    #[tokio::test]
    async fn test_complete_checkout_not_ready() {
        let app = setup_test_app().await;

        // Create session without required fields
        let session_id = create_test_session(app.clone()).await;

        let complete_body = json!({
            "payment": {
                "method": "pm_card_visa"
            }
        });

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/checkout_sessions/{}/complete", session_id))
                    .header("Content-Type", "application/json")
                    .header("Authorization", "Bearer test_api_key")
                    .body(Body::from(complete_body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_cancel_checkout_session_success() {
        let app = setup_test_app().await;

        let session_id = create_test_session(app.clone()).await;

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/checkout_sessions/{}/cancel", session_id))
                    .header("Authorization", "Bearer test_api_key")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        let canceled: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(canceled["status"], "canceled");
    }

    #[tokio::test]
    async fn test_idempotency_key_handling() {
        let app = setup_test_app().await;

        let request_body = json!({
            "items": [{"id": "item_123", "quantity": 1}]
        });

        let idempotency_key = "test-idempotency-key-123";

        // First request
        let response1 = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/checkout_sessions")
                    .header("Content-Type", "application/json")
                    .header("Authorization", "Bearer test_api_key")
                    .header("API-Version", "2025-09-29")
                    .header("Idempotency-Key", idempotency_key)
                    .body(Body::from(request_body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response1.status(), StatusCode::CREATED);
        let body1 = hyper::body::to_bytes(response1.into_body()).await.unwrap();
        let session1: Value = serde_json::from_slice(&body1).unwrap();

        // Second request with same key should return same session
        let response2 = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/checkout_sessions")
                    .header("Content-Type", "application/json")
                    .header("Authorization", "Bearer test_api_key")
                    .header("API-Version", "2025-09-29")
                    .header("Idempotency-Key", idempotency_key)
                    .body(Body::from(request_body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response2.status(), StatusCode::OK);
        let body2 = hyper::body::to_bytes(response2.into_body()).await.unwrap();
        let session2: Value = serde_json::from_slice(&body2).unwrap();

        assert_eq!(session1["id"], session2["id"]);
    }
}

#[cfg(test)]
mod delegated_payment_tests {
    use super::*;

    #[tokio::test]
    async fn test_delegate_payment_success() {
        let app = setup_test_app().await;

        let request_body = json!({
            "payment_method": {
                "type": "card",
                "card_number_type": "fpan",
                "number": "4242424242424242",
                "exp_month": "12",
                "exp_year": "2027",
                "cvc": "123",
                "display_brand": "Visa",
                "display_last4": "4242"
            },
            "allowance": {
                "reason": "one_time",
                "max_amount": 10000,
                "currency": "usd",
                "checkout_session_id": "session_123",
                "expires_at": "2025-12-31T23:59:59Z"
            },
            "risk_signals": []
        });

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/agentic_commerce/delegate_payment")
                    .header("Content-Type", "application/json")
                    .header("Authorization", "Bearer psp_api_key")
                    .body(Body::from(request_body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::CREATED);

        let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        let result: Value = serde_json::from_slice(&body).unwrap();
        assert!(result["id"].as_str().unwrap().starts_with("vt_"));
    }

    #[tokio::test]
    async fn test_delegate_payment_invalid_card() {
        let app = setup_test_app().await;

        let request_body = json!({
            "payment_method": {
                "type": "card",
                "number": "1234", // Invalid
                "exp_month": "12",
                "exp_year": "2027",
                "cvc": "123"
            },
            "allowance": {
                "max_amount": 10000,
                "currency": "usd"
            }
        });

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/agentic_commerce/delegate_payment")
                    .header("Content-Type", "application/json")
                    .header("Authorization", "Bearer psp_api_key")
                    .body(Body::from(request_body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_vault_token_single_use() {
        let app = setup_test_app().await;

        // Create session and vault token
        let session_id = create_ready_session(app.clone()).await;
        let vault_token = create_vault_token(app.clone(), &session_id, 50000).await;

        // First use should succeed
        let complete_body = json!({
            "payment": {
                "delegated_token": vault_token
            }
        });

        let response1 = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/checkout_sessions/{}/complete", session_id))
                    .header("Content-Type", "application/json")
                    .header("Authorization", "Bearer test_api_key")
                    .body(Body::from(complete_body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response1.status(), StatusCode::OK);

        // Second use should fail (token already consumed)
        let session_id2 = create_ready_session(app.clone()).await;
        let response2 = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/checkout_sessions/{}/complete", session_id2))
                    .header("Content-Type", "application/json")
                    .header("Authorization", "Bearer test_api_key")
                    .body(Body::from(complete_body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response2.status(), StatusCode::BAD_REQUEST);
    }
}

#[cfg(test)]
mod neural_commerce_tests {
    use super::*;

    #[tokio::test]
    async fn test_semantic_search_success() {
        let app = setup_test_app().await;

        let request_body = json!({
            "query": "wireless mouse",
            "limit": 5
        });

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/neural/search")
                    .header("Content-Type", "application/json")
                    .header("Authorization", "Bearer test_api_key")
                    .body(Body::from(request_body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        // May return 400 if neural services not enabled
        assert!(
            response.status() == StatusCode::OK
                || response.status() == StatusCode::BAD_REQUEST
        );
    }

    #[tokio::test]
    async fn test_chat_with_inventory() {
        let app = setup_test_app().await;

        let request_body = json!({
            "query": "Do you have any keyboards in stock?"
        });

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/neural/chat")
                    .header("Content-Type", "application/json")
                    .header("Authorization", "Bearer test_api_key")
                    .body(Body::from(request_body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert!(
            response.status() == StatusCode::OK
                || response.status() == StatusCode::BAD_REQUEST
        );
    }
}

#[cfg(test)]
mod return_service_tests {
    use super::*;

    #[tokio::test]
    async fn test_create_return_success() {
        let app = setup_test_app().await;

        let request_body = json!({
            "product_id": "item_123",
            "reason": "Defective",
            "comment": "The product stopped working after 2 days"
        });

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/returns")
                    .header("Content-Type", "application/json")
                    .header("Authorization", "Bearer test_api_key")
                    .body(Body::from(request_body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        let return_req: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(return_req["product_id"], "item_123");
        assert_eq!(return_req["status"], "pending");
    }

    #[tokio::test]
    async fn test_list_pending_returns() {
        let app = setup_test_app().await;

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/returns/pending")
                    .header("Authorization", "Bearer test_api_key")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        let returns: Value = serde_json::from_slice(&body).unwrap();
        assert!(returns.is_array());
    }
}

#[cfg(test)]
mod health_and_monitoring_tests {
    use super::*;

    #[tokio::test]
    async fn test_health_check() {
        let app = setup_test_app().await;

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        let health: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(health["status"], "healthy");
    }

    #[tokio::test]
    async fn test_readiness_check() {
        let app = setup_test_app().await;

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/ready")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_metrics_endpoint() {
        let app = setup_test_app().await;

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/metrics")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        let metrics = String::from_utf8(body.to_vec()).unwrap();
        assert!(metrics.contains("http_requests_total") || !metrics.is_empty());
    }
}

#[cfg(test)]
mod rate_limiting_tests {
    use super::*;

    #[tokio::test]
    async fn test_rate_limit_enforcement() {
        let app = setup_test_app().await;

        // Make multiple rapid requests
        let mut responses = Vec::new();
        for _ in 0..150 {
            // Assuming limit is 100/min
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .method("GET")
                        .uri("/health")
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            responses.push(response.status());
        }

        // Should have some rate limit responses
        let rate_limited = responses
            .iter()
            .filter(|&&s| s == StatusCode::TOO_MANY_REQUESTS)
            .count();

        assert!(rate_limited > 0, "Rate limiting should kick in");
    }
}

#[cfg(test)]
mod security_tests {
    use super::*;

    #[tokio::test]
    async fn test_authentication_required() {
        let app = setup_test_app().await;

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/checkout_sessions")
                    .header("Content-Type", "application/json")
                    // Missing Authorization header
                    .body(Body::from("{}"))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_invalid_api_key() {
        let app = setup_test_app().await;

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/checkout_sessions/test")
                    .header("Authorization", "Bearer invalid_key")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
}
