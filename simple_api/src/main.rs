use axum::{
    extract::State,
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;
use tower_http::cors::CorsLayer;

// Simple in-memory storage for demonstration
#[derive(Clone, Serialize, Deserialize)]
pub struct Order {
    pub id: String,
    pub customer_name: String,
    pub total_amount: f64,
    pub status: String,
    pub created_at: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct InventoryItem {
    pub id: String,
    pub name: String,
    pub quantity: i32,
    pub price: f64,
}

#[derive(Clone)]
pub struct AppState {
    pub orders: Arc<Mutex<Vec<Order>>>,
    pub inventory: Arc<Mutex<Vec<InventoryItem>>>,
}

#[tokio::main]
async fn main() {
    println!("🚀 Starting StateSet API Server...");
    
    // Initialize state
    let state = AppState {
        orders: Arc::new(Mutex::new(Vec::new())),
        inventory: Arc::new(Mutex::new(vec![
            InventoryItem {
                id: "1".to_string(),
                name: "Widget A".to_string(),
                quantity: 100,
                price: 19.99,
            },
            InventoryItem {
                id: "2".to_string(),
                name: "Widget B".to_string(),
                quantity: 50,
                price: 29.99,
            },
        ])),
    };
    
    // Build the router
    let app = Router::new()
        .route("/", get(root))
        .route("/health", get(health_check))
        .route("/api/v1/orders", get(list_orders).post(create_order))
        .route("/api/v1/inventory", get(list_inventory))
        .layer(CorsLayer::permissive())
        .with_state(state);
    
    // Start server
    let addr = "0.0.0.0:8080";
    println!("📡 Server running on http://{}", addr);
    println!("📋 Available endpoints:");
    println!("   GET  /                    - Root endpoint");
    println!("   GET  /health             - Health check");
    println!("   GET  /api/v1/orders      - List orders");
    println!("   POST /api/v1/orders      - Create order");
    println!("   GET  /api/v1/inventory   - List inventory");
    
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

// Root endpoint
async fn root() -> &'static str {
    "🎉 Welcome to StateSet API! Visit /health for status or /api/v1/orders for orders."
}

// Health check
async fn health_check() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "healthy",
        "service": "StateSet API",
        "version": "1.0.0",
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "uptime": "Running"
    }))
}

// List orders
async fn list_orders(State(state): State<AppState>) -> Json<serde_json::Value> {
    let orders = state.orders.lock().await;
    Json(serde_json::json!({
        "orders": orders.clone(),
        "count": orders.len()
    }))
}

// Create order
async fn create_order(
    State(state): State<AppState>,
    Json(payload): Json<serde_json::Value>,
) -> Result<Json<Order>, StatusCode> {
    let customer_name = payload.get("customer_name")
        .and_then(|v| v.as_str())
        .unwrap_or("Unknown Customer");
    
    let total_amount = payload.get("total_amount")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    
    let order = Order {
        id: format!("order_{}", chrono::Utc::now().timestamp()),
        customer_name: customer_name.to_string(),
        total_amount,
        status: "pending".to_string(),
        created_at: chrono::Utc::now().to_rfc3339(),
    };
    
    let mut orders = state.orders.lock().await;
    orders.push(order.clone());
    
    Ok(Json(order))
}

// List inventory
async fn list_inventory(State(state): State<AppState>) -> Json<serde_json::Value> {
    let inventory = state.inventory.lock().await;
    Json(serde_json::json!({
        "inventory": inventory.clone(),
        "count": inventory.len()
    }))
}
