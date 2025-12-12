// This is a completely standalone binary that doesn't depend on any of the library code
use axum::{
    extract::State,
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;
use tower_http::cors::{Any, CorsLayer};
use tracing::info;

// Simple state with a counter
struct AppState {
    counter: Mutex<i32>,
}

// Response model
#[derive(Serialize)]
struct ApiResponse {
    success: bool,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<serde_json::Value>,
}

// Basic request model
#[derive(Deserialize)]
struct CounterRequest {
    increment_by: Option<i32>,
}

// Simple handler for root endpoint
async fn hello() -> Json<ApiResponse> {
    Json(ApiResponse {
        success: true,
        message: "Welcome to Stateset API".to_string(),
        data: None,
    })
}

// Health check endpoint
async fn health_check() -> StatusCode {
    StatusCode::OK
}

// Get the current counter value
async fn get_counter(State(state): State<Arc<AppState>>) -> Json<ApiResponse> {
    let counter = *state.counter.lock().await;

    Json(ApiResponse {
        success: true,
        message: "Current counter value".to_string(),
        data: Some(serde_json::json!({ "value": counter })),
    })
}

// Increment the counter
async fn increment_counter(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<CounterRequest>,
) -> Json<ApiResponse> {
    let increment_by = payload.increment_by.unwrap_or(1);
    let mut counter = state.counter.lock().await;
    *counter += increment_by;

    Json(ApiResponse {
        success: true,
        message: format!("Counter incremented by {}", increment_by),
        data: Some(serde_json::json!({ "value": *counter })),
    })
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing subscriber
    tracing_subscriber::fmt::init();

    info!("Starting Stateset API standalone server");

    // Create app state
    let app_state = Arc::new(AppState {
        counter: Mutex::new(0),
    });

    // Configure CORS
    let cors = CorsLayer::new()
        .allow_methods(Any)
        .allow_origin(Any)
        .allow_headers(Any);

    // Build the router
    let app = Router::new()
        .route("/", get(hello))
        .route("/health", get(health_check))
        .route("/counter", get(get_counter))
        .route("/counter", post(increment_counter))
        .layer(cors)
        .with_state(app_state);

    // Start the server
    let address = "127.0.0.1:3000";
    info!("Listening on {}", address);

    let listener = tokio::net::TcpListener::bind(address).await?;

    // Started message
    info!("Stateset API is running at http://{}", address);
    info!("Available routes:");
    info!("  GET  / - Welcome message");
    info!("  GET  /health - Health check");
    info!("  GET  /counter - Get current counter value");
    info!("  POST /counter - Increment counter");

    // Start server
    axum::serve(listener, app).await?;

    Ok(())
}
