use std::sync::Arc;

use tokio::signal;
use tonic::transport::Server;

use stateset_api::{
    api::StateSetApi,
    config,
    db::{self, DatabaseAccess},
    events::{process_events, EventSender},
    proto::{inventory, order, return_order, shipment, warranty, work_order},
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();
    tracing::info!("Starting StateSet gRPC server...");

    let config = config::load_config()?;

    let db_pool = Arc::new(db::establish_connection_from_app_config(&config).await?);
    tracing::info!("Database connection established");

    if config.auto_migrate || !config.is_production() {
        if let Err(e) = db::run_migrations(&db_pool).await {
            tracing::warn!("Migration warning: {}", e);
        }
    }

    let (tx, rx) = tokio::sync::mpsc::channel(config.event_channel_capacity);
    let event_sender = EventSender::new(tx);
    let event_processor_handle = tokio::spawn(process_events(rx, None, None));

    let db_access = Arc::new(DatabaseAccess::new(db_pool.clone()));
    let grpc_api = StateSetApi::with_event_sender(db_access, db_pool.clone(), event_sender.clone());

    let grpc_port = config.grpc_port.unwrap_or(config.port + 1);
    let grpc_addr = format!("{}:{}", config.host, grpc_port).parse()?;

    tracing::info!(
        "🚀 StateSet gRPC API server listening on grpc://{}",
        grpc_addr
    );

    let grpc_server = Server::builder()
        .add_service(order::order_service_server::OrderServiceServer::new(
            grpc_api.clone(),
        ))
        .add_service(
            inventory::inventory_service_server::InventoryServiceServer::new(grpc_api.clone()),
        )
        .add_service(
            return_order::return_service_server::ReturnServiceServer::new(grpc_api.clone()),
        )
        .add_service(
            warranty::warranty_service_server::WarrantyServiceServer::new(grpc_api.clone()),
        )
        .add_service(
            shipment::shipment_service_server::ShipmentServiceServer::new(grpc_api.clone()),
        )
        .add_service(work_order::work_order_service_server::WorkOrderServiceServer::new(grpc_api))
        .serve_with_shutdown(grpc_addr, shutdown_signal());

    grpc_server.await.map_err(|e| {
        tracing::error!("gRPC server stopped: {:?}", e);
        e
    })?;

    event_processor_handle.abort();
    let _ = event_processor_handle.await;
    tracing::info!("✅ StateSet gRPC server shutdown complete");

    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        if let Err(err) = signal::ctrl_c().await {
            tracing::warn!("Failed to install Ctrl+C handler: {}", err);
            std::future::pending::<()>().await;
        }
    };

    #[cfg(unix)]
    let terminate = async {
        match signal::unix::signal(signal::unix::SignalKind::terminate()) {
            Ok(mut sig) => {
                sig.recv().await;
            }
            Err(err) => {
                tracing::warn!("Failed to install terminate signal handler: {}", err);
                std::future::pending::<()>().await;
            }
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    tracing::info!("Shutdown signal received");
}
