use std::{sync::Arc, time::Duration};
use tokio::signal;
use tonic::transport::Server;

use stateset_api::{
    config, db,
    events::{process_events, EventSender},
    handlers::AppServices,
    proto::*,
    services, AppState,
};

use stateset_api::proto::work_order;
// Import work order service
use stateset_api::proto::work_order::work_order_service_server::{
    WorkOrderService, WorkOrderServiceServer,
};

// Simple placeholder implementation for work orders
pub struct PlaceholderWorkOrderService;

#[tonic::async_trait]
impl WorkOrderService for PlaceholderWorkOrderService {
    async fn create_work_order(
        &self,
        _request: tonic::Request<work_order::CreateWorkOrderRequest>,
    ) -> Result<tonic::Response<work_order::CreateWorkOrderResponse>, tonic::Status> {
        Err(tonic::Status::unimplemented(
            "Work order service not yet implemented",
        ))
    }

    async fn get_work_order(
        &self,
        _request: tonic::Request<work_order::GetWorkOrderRequest>,
    ) -> Result<tonic::Response<work_order::GetWorkOrderResponse>, tonic::Status> {
        Err(tonic::Status::unimplemented(
            "Work order service not yet implemented",
        ))
    }

    async fn update_work_order(
        &self,
        _request: tonic::Request<work_order::UpdateWorkOrderRequest>,
    ) -> Result<tonic::Response<work_order::UpdateWorkOrderResponse>, tonic::Status> {
        Err(tonic::Status::unimplemented(
            "Work order service not yet implemented",
        ))
    }

    async fn list_work_orders(
        &self,
        _request: tonic::Request<work_order::ListWorkOrdersRequest>,
    ) -> Result<tonic::Response<work_order::ListWorkOrdersResponse>, tonic::Status> {
        Err(tonic::Status::unimplemented(
            "Work order service not yet implemented",
        ))
    }

    async fn delete_work_order(
        &self,
        _request: tonic::Request<work_order::DeleteWorkOrderRequest>,
    ) -> Result<tonic::Response<work_order::DeleteWorkOrderResponse>, tonic::Status> {
        Err(tonic::Status::unimplemented(
            "Work order service not yet implemented",
        ))
    }

    async fn assign_work_order(
        &self,
        _request: tonic::Request<work_order::AssignWorkOrderRequest>,
    ) -> Result<tonic::Response<work_order::AssignWorkOrderResponse>, tonic::Status> {
        Err(tonic::Status::unimplemented(
            "Work order service not yet implemented",
        ))
    }

    async fn complete_work_order(
        &self,
        _request: tonic::Request<work_order::CompleteWorkOrderRequest>,
    ) -> Result<tonic::Response<work_order::CompleteWorkOrderResponse>, tonic::Status> {
        Err(tonic::Status::unimplemented(
            "Work order service not yet implemented",
        ))
    }
}

struct OrderGrpcService;

#[tonic::async_trait]
impl order::order_service_server::OrderService for OrderGrpcService {
    async fn create_order(
        &self,
        _request: tonic::Request<order::CreateOrderRequest>,
    ) -> Result<tonic::Response<order::CreateOrderResponse>, tonic::Status> {
        Err(tonic::Status::unimplemented(
            "Order gRPC not yet implemented",
        ))
    }

    async fn get_order(
        &self,
        _request: tonic::Request<order::GetOrderRequest>,
    ) -> Result<tonic::Response<order::GetOrderResponse>, tonic::Status> {
        Err(tonic::Status::unimplemented(
            "Order gRPC not yet implemented",
        ))
    }

    async fn update_order_status(
        &self,
        _request: tonic::Request<order::UpdateOrderStatusRequest>,
    ) -> Result<tonic::Response<order::UpdateOrderStatusResponse>, tonic::Status> {
        Err(tonic::Status::unimplemented(
            "Order gRPC not yet implemented",
        ))
    }

    async fn list_orders(
        &self,
        _request: tonic::Request<order::ListOrdersRequest>,
    ) -> Result<tonic::Response<order::ListOrdersResponse>, tonic::Status> {
        Err(tonic::Status::unimplemented(
            "Order gRPC not yet implemented",
        ))
    }
}

struct InventoryGrpcService;

#[tonic::async_trait]
impl inventory::inventory_service_server::InventoryService for InventoryGrpcService {
    async fn update_inventory(
        &self,
        _request: tonic::Request<inventory::UpdateInventoryRequest>,
    ) -> Result<tonic::Response<inventory::UpdateInventoryResponse>, tonic::Status> {
        Err(tonic::Status::unimplemented(
            "Inventory gRPC not yet implemented",
        ))
    }

    async fn get_inventory(
        &self,
        _request: tonic::Request<inventory::GetInventoryRequest>,
    ) -> Result<tonic::Response<inventory::GetInventoryResponse>, tonic::Status> {
        Err(tonic::Status::unimplemented(
            "Inventory gRPC not yet implemented",
        ))
    }

    async fn list_inventory(
        &self,
        _request: tonic::Request<inventory::ListInventoryRequest>,
    ) -> Result<tonic::Response<inventory::ListInventoryResponse>, tonic::Status> {
        Err(tonic::Status::unimplemented(
            "Inventory gRPC not yet implemented",
        ))
    }

    async fn reserve_inventory(
        &self,
        _request: tonic::Request<inventory::ReserveInventoryRequest>,
    ) -> Result<tonic::Response<inventory::ReserveInventoryResponse>, tonic::Status> {
        Err(tonic::Status::unimplemented(
            "Inventory gRPC not yet implemented",
        ))
    }
}

struct ReturnGrpcService;

#[tonic::async_trait]
impl return_order::return_service_server::ReturnService for ReturnGrpcService {
    async fn create_return(
        &self,
        _request: tonic::Request<return_order::CreateReturnRequest>,
    ) -> Result<tonic::Response<return_order::CreateReturnResponse>, tonic::Status> {
        Err(tonic::Status::unimplemented(
            "Return gRPC not yet implemented",
        ))
    }

    async fn get_return(
        &self,
        _request: tonic::Request<return_order::GetReturnRequest>,
    ) -> Result<tonic::Response<return_order::GetReturnResponse>, tonic::Status> {
        Err(tonic::Status::unimplemented(
            "Return gRPC not yet implemented",
        ))
    }

    async fn update_return_status(
        &self,
        _request: tonic::Request<return_order::UpdateReturnStatusRequest>,
    ) -> Result<tonic::Response<return_order::UpdateReturnStatusResponse>, tonic::Status> {
        Err(tonic::Status::unimplemented(
            "Return gRPC not yet implemented",
        ))
    }

    async fn list_returns(
        &self,
        _request: tonic::Request<return_order::ListReturnsRequest>,
    ) -> Result<tonic::Response<return_order::ListReturnsResponse>, tonic::Status> {
        Err(tonic::Status::unimplemented(
            "Return gRPC not yet implemented",
        ))
    }
}

struct WarrantyGrpcService;

#[tonic::async_trait]
impl warranty::warranty_service_server::WarrantyService for WarrantyGrpcService {
    async fn create_warranty(
        &self,
        _request: tonic::Request<warranty::CreateWarrantyRequest>,
    ) -> Result<tonic::Response<warranty::CreateWarrantyResponse>, tonic::Status> {
        Err(tonic::Status::unimplemented(
            "Warranty gRPC not yet implemented",
        ))
    }

    async fn get_warranty(
        &self,
        _request: tonic::Request<warranty::GetWarrantyRequest>,
    ) -> Result<tonic::Response<warranty::GetWarrantyResponse>, tonic::Status> {
        Err(tonic::Status::unimplemented(
            "Warranty gRPC not yet implemented",
        ))
    }

    async fn update_warranty(
        &self,
        _request: tonic::Request<warranty::UpdateWarrantyRequest>,
    ) -> Result<tonic::Response<warranty::UpdateWarrantyResponse>, tonic::Status> {
        Err(tonic::Status::unimplemented(
            "Warranty gRPC not yet implemented",
        ))
    }

    async fn list_warranties(
        &self,
        _request: tonic::Request<warranty::ListWarrantiesRequest>,
    ) -> Result<tonic::Response<warranty::ListWarrantiesResponse>, tonic::Status> {
        Err(tonic::Status::unimplemented(
            "Warranty gRPC not yet implemented",
        ))
    }
}

struct ShipmentGrpcService;

#[tonic::async_trait]
impl shipment::shipment_service_server::ShipmentService for ShipmentGrpcService {
    async fn create_shipment(
        &self,
        _request: tonic::Request<shipment::CreateShipmentRequest>,
    ) -> Result<tonic::Response<shipment::CreateShipmentResponse>, tonic::Status> {
        Err(tonic::Status::unimplemented(
            "Shipment gRPC not yet implemented",
        ))
    }

    async fn get_shipment(
        &self,
        _request: tonic::Request<shipment::GetShipmentRequest>,
    ) -> Result<tonic::Response<shipment::GetShipmentResponse>, tonic::Status> {
        Err(tonic::Status::unimplemented(
            "Shipment gRPC not yet implemented",
        ))
    }

    async fn update_shipment_status(
        &self,
        _request: tonic::Request<shipment::UpdateShipmentStatusRequest>,
    ) -> Result<tonic::Response<shipment::UpdateShipmentStatusResponse>, tonic::Status> {
        Err(tonic::Status::unimplemented(
            "Shipment gRPC not yet implemented",
        ))
    }

    async fn list_shipments(
        &self,
        _request: tonic::Request<shipment::ListShipmentsRequest>,
    ) -> Result<tonic::Response<shipment::ListShipmentsResponse>, tonic::Status> {
        Err(tonic::Status::unimplemented(
            "Shipment gRPC not yet implemented",
        ))
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

    // Shared Redis client
    let redis_client = Arc::new(redis::Client::open(config.redis_url.clone())?);

    // Core services
    let inventory_service =
        services::inventory::InventoryService::new(db_arc.clone(), event_sender.clone());
    let event_sender_arc = Arc::new(event_sender.clone());

    let auth_service = Arc::new(stateset_api::auth::AuthService::new(
        stateset_api::auth::AuthConfig::new(
            config.jwt_secret.clone(),
            "stateset-api".to_string(),
            "stateset-auth".to_string(),
            Duration::from_secs(config.jwt_expiration as u64),
            Duration::from_secs(config.refresh_token_expiration as u64),
            "sk_".to_string(),
        ),
        db_arc.clone(),
    ));

    let services = AppServices::new(
        db_arc.clone(),
        event_sender_arc.clone(),
        redis_client.clone(),
        auth_service,
    );

    // Create app state
    let _state = AppState {
        db: db_arc.clone(),
        config: config.clone(),
        event_sender: event_sender.clone(),
        inventory_service,
        services,
        redis: redis_client.clone(),
    };

    // Get gRPC port
    let grpc_port = config.grpc_port.unwrap_or(config.port + 1);
    let grpc_addr = format!("{}:{}", config.host, grpc_port).parse()?;

    tracing::info!(
        "🚀 StateSet gRPC API server listening on grpc://{}",
        grpc_addr
    );

    // Start gRPC server
    let grpc_server = Server::builder()
        .add_service(order::order_service_server::OrderServiceServer::new(
            OrderGrpcService,
        ))
        .add_service(
            inventory::inventory_service_server::InventoryServiceServer::new(InventoryGrpcService),
        )
        .add_service(
            return_order::return_service_server::ReturnServiceServer::new(ReturnGrpcService),
        )
        .add_service(
            warranty::warranty_service_server::WarrantyServiceServer::new(WarrantyGrpcService),
        )
        .add_service(
            shipment::shipment_service_server::ShipmentServiceServer::new(ShipmentGrpcService),
        )
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
