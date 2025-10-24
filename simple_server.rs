use axum::{
    Router,
    routing::get,
    response::Json,
    http::StatusCode,
};
use serde_json::json;
use std::net::SocketAddr;
use tower_http::cors::{CorsLayer, Any};
use tracing::{info, error};

async fn health() -> Result<Json<serde_json::Value>, StatusCode> {
    Ok(Json(json!({
        "status": "healthy",
        "service": "stateset-api",
        "version": "0.1.4",
        "timestamp": chrono::Utc::now()
    })))
}

async fn api_info() -> Result<Json<serde_json::Value>, StatusCode> {
    Ok(Json(json!({
        "name": "Stateset API",
        "version": "0.1.4",
        "description": "State-of-the-art supply chain and operations management API",
        "endpoints": {
            "/health": "Health check endpoint",
            "/api/info": "API information"
        }
    })))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    info!("Starting Stateset API simple server...");

    // Build our application with a route
    let app = Router::new()
        .route("/health", get(health))
        .route("/api/info", get(api_info))
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any)
        );

    // Run it
    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));
    info!("Stateset API listening on {}", addr);
    
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;
    
    Ok(())
}
