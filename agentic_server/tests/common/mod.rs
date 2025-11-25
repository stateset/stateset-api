/// Common test utilities and helpers
use axum::Router;
use serde_json::{json, Value};
use std::sync::Arc;

/// Setup test application with all dependencies
pub async fn setup_test_app() -> Router {
    // Initialize test environment
    let _ = dotenvy::dotenv();

    // Create test cache
    let cache = Arc::new(agentic_commerce_server::cache::InMemoryCache::new());

    // Create test event sender
    let (event_tx, _event_rx) = tokio::sync::mpsc::channel(1024);
    let event_sender = Arc::new(agentic_commerce_server::events::EventSender::new(event_tx));

    // Initialize test services
    let product_catalog = Arc::new(agentic_commerce_server::product_catalog::ProductCatalogService::new());
    let tax_service = Arc::new(agentic_commerce_server::tax_service::TaxService::new());
    let delegated_payment_service =
        Arc::new(agentic_commerce_server::delegated_payment::DelegatedPaymentService::new(
            cache.clone(),
        ));
    let return_service = Arc::new(agentic_commerce_server::return_service::ReturnService::new());
    let fraud_service = Arc::new(agentic_commerce_server::fraud_service::FraudService::new());

    let checkout_service = Arc::new(agentic_commerce_server::service::AgenticCheckoutService::new(
        cache.clone(),
        event_sender.clone(),
        product_catalog,
        tax_service,
        None, // No Stripe in tests
        None, // No Shopify in tests
        Some(fraud_service.clone()),
    ));

    let rate_limiter = agentic_commerce_server::rate_limit::RateLimiter::new(1000); // High limit for tests
    let api_key_store = agentic_commerce_server::auth::ApiKeyStore::new();

    let app_state = agentic_commerce_server::AppState {
        checkout_service,
        delegated_payment_service,
        rate_limiter,
        api_key_store,
        signature_verifier: None,
        idempotency_service: None,
        semantic_search_service: None,
        chat_service: None,
        return_service,
        fraud_service,
    };

    // Build test router (simplified version without all middleware)
    Router::new()
        .route("/", axum::routing::get(|| async { "Test Server" }))
        .route("/health", axum::routing::get(health_check))
        .route("/ready", axum::routing::get(readiness_check))
        .route("/metrics", axum::routing::get(metrics_handler))
        .route(
            "/checkout_sessions",
            axum::routing::post(create_checkout_session),
        )
        .route(
            "/checkout_sessions/:id",
            axum::routing::get(get_checkout_session).post(update_checkout_session),
        )
        .route(
            "/checkout_sessions/:id/complete",
            axum::routing::post(complete_checkout_session),
        )
        .route(
            "/checkout_sessions/:id/cancel",
            axum::routing::post(cancel_checkout_session),
        )
        .route(
            "/agentic_commerce/delegate_payment",
            axum::routing::post(delegate_payment),
        )
        .route("/neural/search", axum::routing::post(semantic_search_handler))
        .route("/neural/chat", axum::routing::post(chat_handler))
        .route("/returns", axum::routing::post(create_return_handler))
        .route("/returns/pending", axum::routing::get(list_returns_handler))
        .with_state(app_state)
}

/// Create a basic test checkout session
pub async fn create_test_session(app: Router) -> String {
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    let request_body = json!({
        "items": [{"id": "item_123", "quantity": 1}]
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

    let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let session: Value = serde_json::from_slice(&body).unwrap();
    session["id"].as_str().unwrap().to_string()
}

/// Create a checkout session ready for payment
pub async fn create_ready_session(app: Router) -> String {
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    let request_body = json!({
        "items": [{"id": "item_123", "quantity": 2}],
        "customer": {
            "shipping_address": {
                "name": "Test User",
                "line1": "123 Test St",
                "city": "Test City",
                "region": "CA",
                "postal_code": "94105",
                "country": "US",
                "email": "test@example.com"
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

    let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let session: Value = serde_json::from_slice(&body).unwrap();
    session["id"].as_str().unwrap().to_string()
}

/// Create a vault token for testing
pub async fn create_vault_token(app: Router, session_id: &str, max_amount: i64) -> String {
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

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
            "max_amount": max_amount,
            "currency": "usd",
            "checkout_session_id": session_id,
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

    let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let result: Value = serde_json::from_slice(&body).unwrap();
    result["id"].as_str().unwrap().to_string()
}

// Handler stubs - these would normally be in main.rs
async fn health_check() -> axum::Json<Value> {
    axum::Json(json!({"status": "healthy"}))
}

async fn readiness_check() -> axum::Json<Value> {
    axum::Json(json!({"ready": true}))
}

async fn metrics_handler() -> String {
    "# Metrics\n".to_string()
}

// Note: These handlers would need to be properly imported/implemented
// For now, they're placeholders
async fn create_checkout_session() -> &'static str {
    "stub"
}
async fn get_checkout_session() -> &'static str {
    "stub"
}
async fn update_checkout_session() -> &'static str {
    "stub"
}
async fn complete_checkout_session() -> &'static str {
    "stub"
}
async fn cancel_checkout_session() -> &'static str {
    "stub"
}
async fn delegate_payment() -> &'static str {
    "stub"
}
async fn semantic_search_handler() -> &'static str {
    "stub"
}
async fn chat_handler() -> &'static str {
    "stub"
}
async fn create_return_handler() -> &'static str {
    "stub"
}
async fn list_returns_handler() -> &'static str {
    "stub"
}
