use std::sync::Arc;
use std::time::Duration;

use axum::Router;

use tokio::signal;
use tower::ServiceBuilder;
use tower_http::{
    cors::CorsLayer,
    timeout::TimeoutLayer,
    trace::TraceLayer,
};

use stateset_api::{
    api::StateSetApi,
    config,
    db,
    events::{process_events, EventSender},
    health,
    proto::*,
    services,
    AppState,
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
        db: db_arc,
        config: config.clone(),
        event_sender,
        inventory_service,
    };

    // Create StateSet API for gRPC
    let stateset_api = StateSetApi::new(db_access);

    // Create enhanced API routes
    let api_routes = stateset_api::api_v1_routes().with_state(state.clone());
    
    let app = Router::new()
        // Health routes (no state needed)
        .nest("/health", health::health_routes())
        // API v1 routes with proper state
        .nest("/api/v1", api_routes)
        // Add comprehensive middleware stack
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
                .layer(TimeoutLayer::new(Duration::from_secs(30)))
                .layer(CorsLayer::permissive())
        );

    // Start HTTP server
    let http_addr = format!("{}:{}", config.host, config.port);
    let listener = tokio::net::TcpListener::bind(&http_addr).await?;
    tracing::info!("StateSet HTTP API server listening on {}", http_addr);

    // Start gRPC server
    let grpc_port = config.port + 1; // Use next port for gRPC
    let grpc_addr = format!("{}:{}", config.host, grpc_port).parse()?;
    tracing::info!("StateSet gRPC API server listening on {}", grpc_addr);

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

    // Start both servers
    tokio::try_join!(
        async { http_server.await.map_err(anyhow::Error::from) },
        async { grpc_server.await.map_err(anyhow::Error::from) }
    )?;

    // Clean up
    event_processor_handle.abort();
    tracing::info!("StateSet API server shutdown complete");

    Ok(())
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



