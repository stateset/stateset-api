use axum::{
    body::Body,
    http::{Request, StatusCode},
    Router,
};
use chrono::Utc;
use rust_decimal::Decimal;
use sea_orm::{Database, DatabaseConnection, EntityTrait, Set, ActiveModelTrait, ColumnTrait, QueryFilter};
use serde_json::{json, Value};
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::mpsc;
use tower::ServiceExt;
use uuid::Uuid;

use stateset_api::{
    config,
    db,
    entities::order::{self, Entity as OrderEntity, Model as OrderModel, ActiveModel as OrderActiveModel},
    events::{Event, EventSender},
    handlers::orders::{CreateOrderPayload, OrderHandlerState},
    services::orders::OrderService,
};

/// Test application state
#[derive(Clone)]
struct TestAppState {
    db_pool: Arc<DatabaseConnection>,
    order_service: OrderService,
}

impl OrderHandlerState for TestAppState {
    fn order_service(&self) -> &OrderService {
        &self.order_service
    }
}

async fn setup_test_app() -> (Router, TestAppState) {
    // Load environment variables
    dotenv::dotenv().ok();
    
    // Connect to test database
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/stateset_db".to_string());
    
    let db_pool = Database::connect(&database_url)
        .await
        .expect("Failed to connect to test database");
    
    // Create event channel
    let (sender, _receiver) = mpsc::channel(1000);
    let event_sender = Arc::new(EventSender::new(sender));
    
    // Create order service
    let order_service = OrderService::new(
        Arc::new(db_pool.clone()),
        Some(event_sender)
    );
    
    let app_state = TestAppState {
        db_pool: Arc::new(db_pool),
        order_service,
    };
    
    // Create router with our handlers
    let app = Router::new()
        .route("/orders", axum::routing::post(stateset_api::handlers::orders::create_order::<TestAppState>))
        .route("/orders", axum::routing::get(stateset_api::handlers::orders::list_orders::<TestAppState>))
        .route("/orders/:id", axum::routing::get(stateset_api::handlers::orders::get_order::<TestAppState>))
        .route("/orders/:id/status", axum::routing::put(stateset_api::handlers::orders::update_order_status::<TestAppState>))
        .route("/orders/:id/cancel", axum::routing::post(stateset_api::handlers::orders::cancel_order::<TestAppState>))
        .route("/orders/:id/archive", axum::routing::post(stateset_api::handlers::orders::archive_order::<TestAppState>))
        .with_state(app_state.clone());
    
    (app, app_state)
}

async fn cleanup_test_data(db: &DatabaseConnection) {
    // Clean up test data
    let _ = OrderEntity::delete_many().exec(db).await;
}

#[tokio::test]
async fn test_create_order_endpoint() {
    let (app, state) = setup_test_app().await;
    
    // Clean up any existing data
    cleanup_test_data(&state.db_pool).await;
    
    let customer_id = Uuid::new_v4();
    let order_number = format!("TEST-ORDER-{}", Uuid::new_v4().simple());
    
    let payload = CreateOrderPayload {
        customer_id,
        order_number: order_number.clone(),
        total_amount: Decimal::from_str("99.99").unwrap(),
        currency: "USD".to_string(),
        payment_status: "pending".to_string(),
        fulfillment_status: "unfulfilled".to_string(),
        payment_method: Some("credit_card".to_string()),
        shipping_method: Some("standard".to_string()),
        notes: Some("Test order for integration testing".to_string()),
        shipping_address: Some("123 Test St, Test City, TC 12345".to_string()),
        billing_address: Some("123 Test St, Test City, TC 12345".to_string()),
    };
    
    let request = Request::builder()
        .method("POST")
        .uri("/orders")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&payload).unwrap()))
        .unwrap();
    
    let response = app.oneshot(request).await.unwrap();
    
    assert_eq!(response.status(), StatusCode::CREATED);
    
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let response_data: Value = serde_json::from_slice(&body).unwrap();
    
    assert_eq!(response_data["success"], true);
    assert_eq!(response_data["data"]["order_number"], order_number);
    assert_eq!(response_data["data"]["customer_id"], customer_id.to_string());
    assert_eq!(response_data["data"]["total_amount"], "99.9900");
    assert_eq!(response_data["data"]["currency"], "USD");
    assert_eq!(response_data["data"]["payment_status"], "pending");
    assert_eq!(response_data["data"]["fulfillment_status"], "unfulfilled");
    
    // Verify data was saved in database
    let saved_order = OrderEntity::find()
        .filter(order::Column::OrderNumber.eq(&order_number))
        .one(&*state.db_pool)
        .await
        .unwrap()
        .expect("Order should be saved in database");
    
    assert_eq!(saved_order.order_number, order_number);
    assert_eq!(saved_order.customer_id, customer_id);
    assert_eq!(saved_order.total_amount, Decimal::from_str("99.99").unwrap());
    assert_eq!(saved_order.currency, "USD");
    
    println!("✅ CREATE order endpoint test PASSED - Order created and saved to PostgreSQL");
}

#[tokio::test]
async fn test_get_order_endpoint() {
    let (app, state) = setup_test_app().await;
    
    // Clean up any existing data
    cleanup_test_data(&state.db_pool).await;
    
    // First create a test order directly in the database
    let order_id = Uuid::new_v4();
    let customer_id = Uuid::new_v4();
    let order_number = format!("TEST-GET-{}", Uuid::new_v4().simple());
    
    let test_order = OrderActiveModel {
        id: Set(order_id),
        customer_id: Set(customer_id),
        order_number: Set(order_number.clone()),
        total_amount: Set(Decimal::from_str("149.99").unwrap()),
        currency: Set("USD".to_string()),
        payment_status: Set("paid".to_string()),
        fulfillment_status: Set("fulfilled".to_string()),
        payment_method: Set(Some("paypal".to_string())),
        shipping_method: Set(Some("express".to_string())),
        notes: Set(Some("Test order for GET endpoint".to_string())),
        status: Set("completed".to_string()),
        is_archived: Set(false),
        version: Set(1),
        created_at: Set(Utc::now()),
        updated_at: Set(Some(Utc::now())),
        ..Default::default()
    };
    
    test_order.insert(&*state.db_pool).await.unwrap();
    
    // Test GET request
    let request = Request::builder()
        .method("GET")
        .uri(&format!("/orders/{}", order_id))
        .body(Body::empty())
        .unwrap();
    
    let response = app.oneshot(request).await.unwrap();
    
    assert_eq!(response.status(), StatusCode::OK);
    
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let response_data: Value = serde_json::from_slice(&body).unwrap();
    
    assert_eq!(response_data["success"], true);
    assert_eq!(response_data["data"]["id"], order_id.to_string());
    assert_eq!(response_data["data"]["order_number"], order_number);
    assert_eq!(response_data["data"]["customer_id"], customer_id.to_string());
    assert_eq!(response_data["data"]["total_amount"], "149.9900");
    assert_eq!(response_data["data"]["payment_status"], "paid");
    assert_eq!(response_data["data"]["fulfillment_status"], "fulfilled");
    assert_eq!(response_data["data"]["status"], "completed");
    
    println!("✅ GET order endpoint test PASSED - Order retrieved from PostgreSQL");
}

#[tokio::test]
async fn test_list_orders_endpoint() {
    let (app, state) = setup_test_app().await;
    
    // Clean up any existing data
    cleanup_test_data(&state.db_pool).await;
    
    // Create multiple test orders
    for i in 1..=5 {
        let order_id = Uuid::new_v4();
        let customer_id = Uuid::new_v4();
        let order_number = format!("TEST-LIST-{:03}", i);
        
        let test_order = OrderActiveModel {
            id: Set(order_id),
            customer_id: Set(customer_id),
            order_number: Set(order_number),
            total_amount: Set(Decimal::from_str(&format!("{}.99", 10 * i)).unwrap()),
            currency: Set("USD".to_string()),
            payment_status: Set("pending".to_string()),
            fulfillment_status: Set("unfulfilled".to_string()),
            status: Set("pending".to_string()),
            is_archived: Set(false),
            version: Set(1),
            created_at: Set(Utc::now()),
            ..Default::default()
        };
        
        test_order.insert(&*state.db_pool).await.unwrap();
    }
    
    // Test LIST request with pagination
    let request = Request::builder()
        .method("GET")
        .uri("/orders?page=1&per_page=3")
        .body(Body::empty())
        .unwrap();
    
    let response = app.oneshot(request).await.unwrap();
    
    assert_eq!(response.status(), StatusCode::OK);
    
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let response_data: Value = serde_json::from_slice(&body).unwrap();
    
    assert_eq!(response_data["success"], true);
    assert_eq!(response_data["data"]["total"], 5);
    assert_eq!(response_data["data"]["page"], 1);
    assert_eq!(response_data["data"]["per_page"], 3);
    assert_eq!(response_data["data"]["orders"].as_array().unwrap().len(), 3);
    
    // Verify the orders contain expected data
    let orders = response_data["data"]["orders"].as_array().unwrap();
    for order in orders {
        assert!(order["order_number"].as_str().unwrap().starts_with("TEST-LIST-"));
        assert_eq!(order["currency"], "USD");
        assert_eq!(order["payment_status"], "pending");
    }
    
    println!("✅ LIST orders endpoint test PASSED - Orders retrieved with pagination from PostgreSQL");
}

#[tokio::test]
async fn test_update_order_status_endpoint() {
    let (app, state) = setup_test_app().await;
    
    // Clean up any existing data
    cleanup_test_data(&state.db_pool).await;
    
    // Create a test order
    let order_id = Uuid::new_v4();
    let customer_id = Uuid::new_v4();
    let order_number = format!("TEST-UPDATE-{}", Uuid::new_v4().simple());
    
    let test_order = OrderActiveModel {
        id: Set(order_id),
        customer_id: Set(customer_id),
        order_number: Set(order_number.clone()),
        total_amount: Set(Decimal::from_str("75.50").unwrap()),
        currency: Set("USD".to_string()),
        payment_status: Set("paid".to_string()),
        fulfillment_status: Set("unfulfilled".to_string()),
        status: Set("pending".to_string()),
        is_archived: Set(false),
        version: Set(1),
        created_at: Set(Utc::now()),
        ..Default::default()
    };
    
    test_order.insert(&*state.db_pool).await.unwrap();
    
    // Test UPDATE status request
    let update_payload = json!({
        "status": "processing",
        "notes": "Order is now being processed"
    });
    
    let request = Request::builder()
        .method("PUT")
        .uri(&format!("/orders/{}/status", order_id))
        .header("content-type", "application/json")
        .body(Body::from(update_payload.to_string()))
        .unwrap();
    
    let response = app.oneshot(request).await.unwrap();
    
    assert_eq!(response.status(), StatusCode::OK);
    
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let response_data: Value = serde_json::from_slice(&body).unwrap();
    
    assert_eq!(response_data["success"], true);
    assert_eq!(response_data["data"]["status"], "processing");
    
    // Verify the status was updated in the database
    let updated_order = OrderEntity::find_by_id(order_id)
        .one(&*state.db_pool)
        .await
        .unwrap()
        .expect("Order should exist in database");
    
    assert_eq!(updated_order.status, "processing");
    assert!(updated_order.updated_at.is_some());
    
    println!("✅ UPDATE order status endpoint test PASSED - Order status updated in PostgreSQL");
}

#[tokio::test]
async fn test_cancel_order_endpoint() {
    let (app, state) = setup_test_app().await;
    
    // Clean up any existing data
    cleanup_test_data(&state.db_pool).await;
    
    // Create a test order
    let order_id = Uuid::new_v4();
    let customer_id = Uuid::new_v4();
    let order_number = format!("TEST-CANCEL-{}", Uuid::new_v4().simple());
    
    let test_order = OrderActiveModel {
        id: Set(order_id),
        customer_id: Set(customer_id),
        order_number: Set(order_number.clone()),
        total_amount: Set(Decimal::from_str("199.99").unwrap()),
        currency: Set("USD".to_string()),
        payment_status: Set("pending".to_string()),
        fulfillment_status: Set("unfulfilled".to_string()),
        status: Set("pending".to_string()),
        is_archived: Set(false),
        version: Set(1),
        created_at: Set(Utc::now()),
        ..Default::default()
    };
    
    test_order.insert(&*state.db_pool).await.unwrap();
    
    // Test CANCEL request
    let cancel_payload = json!({
        "reason": "Customer requested cancellation"
    });
    
    let request = Request::builder()
        .method("POST")
        .uri(&format!("/orders/{}/cancel", order_id))
        .header("content-type", "application/json")
        .body(Body::from(cancel_payload.to_string()))
        .unwrap();
    
    let response = app.oneshot(request).await.unwrap();
    
    assert_eq!(response.status(), StatusCode::OK);
    
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let response_data: Value = serde_json::from_slice(&body).unwrap();
    
    assert_eq!(response_data["success"], true);
    assert_eq!(response_data["data"]["status"], "cancelled");
    
    // Verify the order was cancelled in the database
    let cancelled_order = OrderEntity::find_by_id(order_id)
        .one(&*state.db_pool)
        .await
        .unwrap()
        .expect("Order should exist in database");
    
    assert_eq!(cancelled_order.status, "cancelled");
    
    println!("✅ CANCEL order endpoint test PASSED - Order cancelled in PostgreSQL");
}

#[tokio::test]
async fn test_data_persistence_verification() {
    let (_, state) = setup_test_app().await;
    
    // Clean up any existing data
    cleanup_test_data(&state.db_pool).await;
    
    // Test direct database operations to verify persistence
    let order_id = Uuid::new_v4();
    let customer_id = Uuid::new_v4();
    let order_number = format!("TEST-PERSIST-{}", Uuid::new_v4().simple());
    
    // Create order using our service
    let create_request = stateset_api::services::orders::CreateOrderRequest {
        customer_id,
        order_number: order_number.clone(),
        total_amount: Decimal::from_str("299.99").unwrap(),
        currency: "EUR".to_string(),
        payment_status: "paid".to_string(),
        fulfillment_status: "fulfilled".to_string(),
        payment_method: Some("bank_transfer".to_string()),
        shipping_method: Some("overnight".to_string()),
        notes: Some("Persistence verification test".to_string()),
        shipping_address: Some("456 Persistence Ave, Test City, TC 54321".to_string()),
        billing_address: Some("456 Persistence Ave, Test City, TC 54321".to_string()),
    };
    
    let created_order = state.order_service.create_order(create_request)
        .await
        .expect("Order creation should succeed");
    
    // Verify the order exists in PostgreSQL
    let db_order = OrderEntity::find()
        .filter(order::Column::OrderNumber.eq(&order_number))
        .one(&*state.db_pool)
        .await
        .unwrap()
        .expect("Order should be persisted in PostgreSQL");
    
    assert_eq!(db_order.order_number, order_number);
    assert_eq!(db_order.customer_id, customer_id);
    assert_eq!(db_order.total_amount, Decimal::from_str("299.99").unwrap());
    assert_eq!(db_order.currency, "EUR");
    assert_eq!(db_order.payment_status, "paid");
    assert_eq!(db_order.fulfillment_status, "fulfilled");
    assert_eq!(db_order.payment_method, Some("bank_transfer".to_string()));
    assert_eq!(db_order.shipping_method, Some("overnight".to_string()));
    assert_eq!(db_order.notes, Some("Persistence verification test".to_string()));
    
    // Update the order and verify persistence
    let update_request = stateset_api::services::orders::UpdateOrderStatusRequest {
        status: "shipped".to_string(),
        notes: Some("Order has been shipped".to_string()),
    };
    
    let updated_order = state.order_service.update_order_status(created_order.id, update_request)
        .await
        .expect("Order update should succeed");
    
    assert_eq!(updated_order.status, "shipped");
    
    // Verify update was persisted
    let updated_db_order = OrderEntity::find_by_id(created_order.id)
        .one(&*state.db_pool)
        .await
        .unwrap()
        .expect("Updated order should be in PostgreSQL");
    
    assert_eq!(updated_db_order.status, "shipped");
    assert!(updated_db_order.updated_at.is_some());
    
    println!("✅ DATA PERSISTENCE test PASSED - All operations properly persisted to PostgreSQL");
}