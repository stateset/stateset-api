use std::net::SocketAddr;
use axum::{Router, routing::get, Json, response::IntoResponse};
use serde_json::json;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting simple test server...");
    
    let app = Router::new()
        .route("/health", get(health_check))
        .route("/", get(|| async { "Hello from StateSet API!" }));
    
    let addr = SocketAddr::from(([127, 0, 0, 1], 8080));
    println!("Server listening on http://{}", addr);
    
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app.into_make_service()).await?;
    
    Ok(())
}

async fn health_check() -> impl IntoResponse {
    Json(json!({
        "status": "healthy",
        "message": "StateSet API is running successfully!",
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "version": "1.0.0"
    }))
}
