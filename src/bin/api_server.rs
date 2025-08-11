use axum::{
    extract::Json,
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Router,
};
use serde::Serialize;
use serde_json::json;
use std::sync::Arc;
use tokio::net::TcpListener;

// Import the stateset-api library for database connection
use stateset_api::{
    config,
    db,
};
use tracing::info;

// Simple app state for this server
#[derive(Clone)]
struct SimpleAppState {
    db: Arc<sea_orm::DatabaseConnection>,
}

// Root handler
async fn root() -> impl IntoResponse {
    Json(json!({
        "name": "Stateset API",
        "version": env!("CARGO_PKG_VERSION"),
        "status": "healthy",
        "documentation": "/docs"
    }))
}

// Health check handler
async fn health_check() -> impl IntoResponse {
    Json(json!({
        "status": "healthy",
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "service": "stateset-api"
    }))
}

// Orders placeholder - just returns empty list for now
async fn list_orders() -> impl IntoResponse {
    Json(json!({
        "success": true,
        "data": {
            "items": [],
            "total": 0,
            "page": 1,
            "limit": 20
        }
    }))
}

// Setup API routes
fn api_routes() -> Router<SimpleAppState> {
    Router::new()
        .route("/", get(root))
        .route("/health", get(health_check))
        .route("/api/v1/orders", get(list_orders))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load environment variables
    dotenv::dotenv().ok();

    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Load configuration
    let config = config::load_config()?;

    // Initialize the database
    let db_pool = db::establish_connection(&config.database_url).await?;
    
    // Initialize simple app state
    let app_state = SimpleAppState {
        db: Arc::new(db_pool),
    };

    // Build the API router
    let app = Router::new()
        .merge(api_routes())
        .with_state(app_state);

    // Start the server
    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], config.port));
    info!("🚀 Stateset API server starting on {}", addr);
    
    let listener = TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}