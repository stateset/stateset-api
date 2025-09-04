use std::sync::Arc;
use tokio::signal;
use tonic::transport::Server;

use stateset_api::{
    config,
    db,
    events::{process_events, EventSender},
    grpc,
    handlers::AppServices,
    proto::*,
    services,
    AppState,
};

// Import work order service
use stateset_api::proto::work_order::work_order_service_server::{WorkOrderService, WorkOrderServiceServer};

// Simple placeholder implementation for work orders
pub struct PlaceholderWorkOrderService;

#[tonic::async_trait]
impl WorkOrderService for PlaceholderWorkOrderService {
    async fn create_work_order(
        &self,
        _request: tonic::Request<CreateWorkOrderRequest>,
    ) -> Result<tonic::Response<CreateWorkOrderResponse>, tonic::Status> {
        Err(tonic::Status::unimplemented("Work order service not yet implemented"))
    }

    async fn get_work_order(
        &self,
        _request: tonic::Request<GetWorkOrderRequest>,
    ) -> Result<tonic::Response<GetWorkOrderResponse>, tonic::Status> {
        Err(tonic::Status::unimplemented("Work order service not yet implemented"))
    }

    async fn update_work_order(
        &self,
        _request: tonic::Request<UpdateWorkOrderRequest>,
    ) -> Result<tonic::Response<UpdateWorkOrderResponse>, tonic::Status> {
        Err(tonic::Status::unimplemented("Work order service not yet implemented"))
    }

    async fn list_work_orders(
        &self,
        _request: tonic::Request<ListWorkOrdersRequest>,
    ) -> Result<tonic::Response<ListWorkOrdersResponse>, tonic::Status> {
        Err(tonic::Status::unimplemented("Work order service not yet implemented"))
    }

    async fn delete_work_order(
        &self,
        _request: tonic::Request<DeleteWorkOrderRequest>,
    ) -> Result<tonic::Response<DeleteWorkOrderResponse>, tonic::Status> {
        Err(tonic::Status::unimplemented("Work order service not yet implemented"))
    }

    async fn assign_work_order(
        &self,
        _request: tonic::Request<AssignWorkOrderRequest>,
    ) -> Result<tonic::Response<AssignWorkOrderResponse>, tonic::Status> {
        Err(tonic::Status::unimplemented("Work order service not yet implemented"))
    }

    async fn complete_work_order(
        &self,
        _request: tonic::Request<CompleteWorkOrderRequest>,
    ) -> Result<tonic::Response<CompleteWorkOrderResponse>, tonic::Status> {
        Err(tonic::Status::unimplemented("Work order service not yet implemented"))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt::init();
    
    tracing::info!("Starting StateSet gRPC server...");
    
    // Load configuration
    let config = config::load_config()?;
    
    // Initialize database connection
    let db_arc = Arc::new(db::establish_connection_from_app_config(&config).await?);
    tracing::info!("Database connection established");
    
    // Run database migrations in development
    if config.auto_migrate || !config.is_production() {
        if let Err(e) = db::run_migrations(&db_arc).await {
            tracing::warn!("Migration warning: {}", e);
        }
    }
    
    // Initialize event system
    let (tx, rx) = tokio::sync::mpsc::channel(1000);
    let event_sender = EventSender::new(tx);
    
    // Start event processing in background
    let event_processor_handle = tokio::spawn(process_events(rx));
    
    // Create services
    let inventory_service = services::inventory::InventoryService::new(
        db_arc.clone(),
        event_sender.clone(),
    );
    
    let order_service = services::orders::OrderService::new(
        db_arc.clone(),
        Some(Arc::new(event_sender.clone())),
    );
    
    // Create app state
    let state = AppState {
        db: db_arc.clone(),
        config: config.clone(),
        event_sender: event_sender.clone(),
        inventory_service,
        services: AppServices {
            product_catalog: Arc::new(services::commerce::ProductCatalogService::new(
                db_arc.clone(),
                event_sender.clone(),
            )),
            cart: Arc::new(services::commerce::CartService::new(
                db_arc.clone(),
                event_sender.clone(),
            )),
            checkout: Arc::new(services::commerce::CheckoutService::new(
                db_arc.clone(),
                event_sender.clone(),
                order_service.clone(),
            )),
            customer: Arc::new(services::commerce::CustomerService::new(
                db_arc.clone(),
                event_sender.clone(),
                Arc::new(stateset_api::auth::AuthService::new(
                    stateset_api::auth::AuthConfig::new(
                        config.jwt_secret.clone(),
                        "stateset-api".to_string(),
                        "stateset-auth".to_string(),
                        std::time::Duration::from_secs(config.jwt_expiration as u64),
                        std::time::Duration::from_secs(config.refresh_token_expiration as u64),
                        "sk_".to_string(),
                    ),
                    db_arc.clone(),
                )),
            )),
            order: order_service,
        },
        redis: Arc::new(redis::Client::open(config.redis_url.clone())?),
    };
    
    // Get gRPC port
    let grpc_port = config.grpc_port.unwrap_or(config.port + 1);
    let grpc_addr = format!("{}:{}", config.host, grpc_port).parse()?;
    
    tracing::info!("🚀 StateSet gRPC API server listening on grpc://{}", grpc_addr);
    
    // Start gRPC server
    let grpc_server = Server::builder()
        .add_service(order::order_service_server::OrderServiceServer::new(
            grpc::OrderGrpcService { svc: state.services.order.clone() }
        ))
        .add_service(inventory::inventory_service_server::InventoryServiceServer::new(
            grpc::InventoryGrpcService { svc: state.inventory_service.clone() }
        ))
        .add_service(return_order::return_service_server::ReturnServiceServer::new(
            grpc::ReturnGrpcService { svc: services::returns::ReturnService::new(db_arc.clone()) }
        ))
        .add_service(warranty::warranty_service_server::WarrantyServiceServer::new(
            grpc::WarrantyGrpcService { svc: services::warranties::WarrantyService::new(db_arc.clone()) }
        ))
        .add_service(shipment::shipment_service_server::ShipmentServiceServer::new(
            grpc::ShipmentGrpcService { svc: services::shipments::ShipmentService::new(db_arc.clone()) }
        ))
        .add_service(WorkOrderServiceServer::new(PlaceholderWorkOrderService))
        .serve_with_shutdown(grpc_addr, shutdown_signal());
    
    // Handle shutdown
    tokio::select! {
        res = grpc_server => {
            tracing::error!("gRPC server stopped: {:?}", res);
            res.map_err(anyhow::Error::from)
        }
        _ = shutdown_signal() => {
            tracing::info!("Graceful shutdown initiated");
            Ok(())
        }
    }?;
    
    // Clean up
    event_processor_handle.abort();
    let _ = event_processor_handle.await;
    tracing::info!("✅ StateSet gRPC server shutdown complete");
    
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
