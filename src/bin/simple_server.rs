use axum::{
    response::{IntoResponse, Json},
    routing::get,
    Router,
};
use serde_json::json;
use std::net::SocketAddr;
use tracing::info;

/// Basic health check response
async fn health_check() -> impl IntoResponse {
    info!("Health check endpoint called");

    // Print detailed information
    tracing::debug!("Serving health check request");

    Json(json!({
        "status": "up",
        "version": env!("CARGO_PKG_VERSION"),
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "server": "Stateset Simple API Server"
    }))
}

/// Root endpoint response
async fn root() -> impl IntoResponse {
    info!("Root endpoint called");

    Json(json!({
        "name": "Stateset API",
        "version": env!("CARGO_PKG_VERSION"),
        "status": "online",
        "message": "Welcome to Stateset API"
    }))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    info!("Stateset Simple API Server starting...");

    // Set up all API routes
    let app = Router::new()
        .route("/", get(root))
        // Health routes
        .route("/health", get(health_check));

    // Start the server
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    info!("Listening on {}", addr);

    axum::serve(tokio::net::TcpListener::bind(addr).await?, app).await?;

    Ok(())
}
