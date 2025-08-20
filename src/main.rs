use std::sync::Arc;
use std::time::Duration;

use axum::{Router, http::StatusCode, Json};
use serde_json::json;
use tokio::signal;
use tower::ServiceBuilder;
use tower_http::{
    cors::CorsLayer,
    timeout::TimeoutLayer,
    trace::TraceLayer,
    compression::CompressionLayer,
};
use tracing::instrument;

use stateset_api::{
    api::StateSetApi,
    config,
    db,
    events::{process_events, EventSender},
    health,
    proto::*,
    services,
    AppState,
    openapi,
    rate_limiter::{RateLimitConfig, RateLimitLayer},
    tracing::RequestLoggerLayer,
    versioning,
};
use tonic::transport::Server;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    tracing::info!("Starting StateSet API server...");

    // Load configuration
    let config = config::load_config()?;
    tracing::info!("Configuration loaded successfully");

    // Initialize database connection
    let db_arc = Arc::new(db::establish_connection(&config.database_url).await?);
    tracing::info!("Database connection established");

    // Run database migrations if enabled
    if config.auto_migrate {
        if let Err(e) = db::run_migrations(&db_arc).await {
            tracing::warn!("Migration warning: {}", e);
        }
    }

    // Initialize event system
    let (tx, rx) = tokio::sync::mpsc::channel(1000);
    let event_sender = EventSender::new(tx);

    // Start event processing in background
    let event_processor_handle = tokio::spawn(process_events(rx));

    // Create database access wrapper
    let db_access = Arc::new(db::DatabaseAccess::new(db_arc.clone()));
    
    // Create inventory service
    let inventory_service = services::inventory::InventoryService::new(
        db_arc.clone(),
        event_sender.clone(),
    );
    
    // Create application state for HTTP API
    let state = AppState {
        db: db_arc.clone(),
        config: config.clone(),
        event_sender: event_sender.clone(),
        inventory_service,
    };

    // Create StateSet API for gRPC with shared event sender
    let stateset_api = StateSetApi::with_event_sender(db_access, db_arc.clone(), event_sender.clone());

    // Create enhanced API routes
    let api_routes = stateset_api::api_v1_routes().with_state(state.clone());
    
    let app = Router::new()
        // Health routes (no state needed)
        .nest("/health", health::health_routes())
        // API versions info
        .nest("/api/versions", versioning::api_versions_routes())
        // Swagger UI and OpenAPI JSON
        .merge(openapi::swagger_routes())
        .nest("/api-docs", openapi::create_docs_routes())
        // Metrics endpoint
        .route("/metrics", axum::routing::get(metrics_endpoint))
        // API v1 routes with proper state
        .nest("/api/v1", api_routes)
        // Fallback 404 JSON
        .fallback(fallback_handler)
        // Add comprehensive middleware stack
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
                .layer(TimeoutLayer::new(Duration::from_secs(30)))
                .layer(CorsLayer::permissive())
                .layer(CompressionLayer::new())
        )
        // Add request ID middleware for better tracing
        .layer(axum::middleware::from_fn(stateset_api::middleware_helpers::request_id_middleware))
        // Add input sanitization
        .layer(axum::middleware::from_fn(stateset_api::middleware_helpers::sanitize_middleware))
        // Add structured request logging
        .layer(RequestLoggerLayer::new())
        // Add simple in-memory rate limiting
        .layer(RateLimitLayer::new(RateLimitConfig::default()))
        // API version header middleware
        .layer(axum::middleware::from_fn(stateset_api::versioning::api_version_middleware));

    // Configure server addresses
    let http_addr = format!("{}:{}", config.host, config.port);
    let grpc_port = config.grpc_port.unwrap_or(config.port + 1);
    let grpc_addr = format!("{}:{}", config.host, grpc_port).parse()?;
    
    // Start HTTP server
    let listener = tokio::net::TcpListener::bind(&http_addr).await?;
    tracing::info!("🚀 StateSet HTTP API server listening on http://{}", http_addr);
    
    // Start gRPC server
    tracing::info!("🚀 StateSet gRPC API server listening on grpc://{}", grpc_addr);

    let grpc_server = Server::builder()
        .add_service(order::order_service_server::OrderServiceServer::new(stateset_api.clone()))
        .add_service(inventory::inventory_service_server::InventoryServiceServer::new(stateset_api.clone()))
        .add_service(return_order::return_service_server::ReturnServiceServer::new(stateset_api.clone()))
        .add_service(warranty::warranty_service_server::WarrantyServiceServer::new(stateset_api.clone()))
        .add_service(shipment::shipment_service_server::ShipmentServiceServer::new(stateset_api.clone()))
        .add_service(work_order::work_order_service_server::WorkOrderServiceServer::new(stateset_api))
        .serve_with_shutdown(grpc_addr, shutdown_signal());

    // Run both servers concurrently
    let http_server = axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal());

    // Start both servers with proper error handling
    let result = tokio::select! {
        res = http_server => {
            tracing::error!("HTTP server stopped: {:?}", res);
            res.map_err(anyhow::Error::from)
        }
        res = grpc_server => {
            tracing::error!("gRPC server stopped: {:?}", res);
            res.map_err(anyhow::Error::from)
        }
        _ = shutdown_signal() => {
            tracing::info!("Graceful shutdown initiated");
            Ok(())
        }
    };

    // Clean up
    event_processor_handle.abort();
    let _ = event_processor_handle.await;
    tracing::info!("✅ StateSet API server shutdown complete");

    result
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    tracing::info!("Shutdown signal received");
}

#[instrument]
async fn metrics_endpoint() -> Result<String, (StatusCode, String)> {
    stateset_api::metrics::metrics_handler()
        .await
        .map_err(|e| {
            tracing::error!("Metrics handler error: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, format!("Metrics export failed: {}", e))
        })
}

#[instrument]
async fn fallback_handler() -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::NOT_FOUND,
        Json(json!({
            "error": "not_found",
            "message": "The requested resource was not found",
            "status": 404,
            "timestamp": chrono::Utc::now().to_rfc3339()
        }))
    )
}