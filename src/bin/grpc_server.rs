use std::{sync::Arc, time::Duration};

use http::header::HeaderName;
use tokio::signal;
use tonic::{codec::CompressionEncoding, transport::Server};
use tower::ServiceBuilder;
use tower_http::{
    request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer},
    trace::TraceLayer,
};

use stateset_api::{
    api::StateSetApi,
    config,
    db::{self, DatabaseAccess},
    events::{process_events, EventSender},
    proto::{inventory, order, return_order, shipment, warranty, work_order},
    tracing::RequestSpanMaker,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cfg = config::load_config()?;
    config::init_tracing(cfg.log_level(), cfg.log_json);
    tracing::info!("Starting StateSet gRPC server...");

    let db_pool = Arc::new(db::establish_connection_from_app_config(&cfg).await?);
    tracing::info!("Database connection established");

    if cfg.auto_migrate {
        if let Err(e) = db::run_migrations(&db_pool).await {
            tracing::warn!("Migration warning: {}", e);
        }
    }

    let (tx, rx) = tokio::sync::mpsc::channel(cfg.event_channel_capacity);
    let event_sender = EventSender::new(tx);
    let event_processor_handle = tokio::spawn(process_events(rx, None, None));

    let db_access = Arc::new(DatabaseAccess::new(db_pool.clone()));
    let grpc_api = StateSetApi::with_event_sender(db_access, db_pool.clone(), event_sender.clone());

    let grpc_port = cfg.grpc_port.unwrap_or(cfg.port + 1);
    let grpc_addr = format!("{}:{}", cfg.host, grpc_port).parse()?;

    tracing::info!(
        "🚀 StateSet gRPC API server listening on grpc://{}",
        grpc_addr
    );

    let x_request_id = HeaderName::from_static("x-request-id");
    let middleware = ServiceBuilder::new()
        .layer(SetRequestIdLayer::new(
            x_request_id.clone(),
            MakeRequestUuid,
        ))
        .layer(TraceLayer::new_for_grpc().make_span_with(RequestSpanMaker::default()))
        .layer(PropagateRequestIdLayer::new(x_request_id))
        .into_inner();

    let order_svc = order::order_service_server::OrderServiceServer::new(grpc_api.clone())
        .accept_compressed(CompressionEncoding::Gzip)
        .send_compressed(CompressionEncoding::Gzip);
    let inventory_svc =
        inventory::inventory_service_server::InventoryServiceServer::new(grpc_api.clone())
            .accept_compressed(CompressionEncoding::Gzip)
            .send_compressed(CompressionEncoding::Gzip);
    let return_svc =
        return_order::return_service_server::ReturnServiceServer::new(grpc_api.clone())
            .accept_compressed(CompressionEncoding::Gzip)
            .send_compressed(CompressionEncoding::Gzip);
    let warranty_svc =
        warranty::warranty_service_server::WarrantyServiceServer::new(grpc_api.clone())
            .accept_compressed(CompressionEncoding::Gzip)
            .send_compressed(CompressionEncoding::Gzip);
    let shipment_svc =
        shipment::shipment_service_server::ShipmentServiceServer::new(grpc_api.clone())
            .accept_compressed(CompressionEncoding::Gzip)
            .send_compressed(CompressionEncoding::Gzip);
    let work_order_svc =
        work_order::work_order_service_server::WorkOrderServiceServer::new(grpc_api)
            .accept_compressed(CompressionEncoding::Gzip)
            .send_compressed(CompressionEncoding::Gzip);

    let concurrency_limit = cfg.grpc_concurrency_limit_per_connection.max(1);

    let mut grpc_builder = Server::builder()
        .layer(middleware)
        .concurrency_limit_per_connection(concurrency_limit);

    if cfg.grpc_timeout_secs > 0 {
        grpc_builder = grpc_builder.timeout(Duration::from_secs(cfg.grpc_timeout_secs));
    }
    if cfg.grpc_tcp_keepalive_secs > 0 {
        grpc_builder =
            grpc_builder.tcp_keepalive(Some(Duration::from_secs(cfg.grpc_tcp_keepalive_secs)));
    }
    if cfg.grpc_http2_keepalive_interval_secs > 0 {
        grpc_builder = grpc_builder.http2_keepalive_interval(Some(Duration::from_secs(
            cfg.grpc_http2_keepalive_interval_secs,
        )));
    }
    if cfg.grpc_http2_keepalive_timeout_secs > 0 {
        grpc_builder = grpc_builder.http2_keepalive_timeout(Some(Duration::from_secs(
            cfg.grpc_http2_keepalive_timeout_secs,
        )));
    }

    let grpc_server = grpc_builder
        .add_service(order_svc)
        .add_service(inventory_svc)
        .add_service(return_svc)
        .add_service(warranty_svc)
        .add_service(shipment_svc)
        .add_service(work_order_svc)
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
