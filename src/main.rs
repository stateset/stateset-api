mod auth;
mod config;
mod db;
mod errors;
mod health;
mod handlers;
mod entities;
mod repositories;
// Using the migrations crate now instead of internal migrator

use axum::{
    Router,
    routing::get,
    extract::Extension,
};
use tower::ServiceBuilder;
use tower_http::{
    trace::TraceLayer,
    cors::{CorsLayer, Any},
};
use dotenv::dotenv;
use sea_orm::DatabaseConnection;
use std::net::SocketAddr;
use std::sync::Arc;
use tracing::{info, error};
use crate::errors::AppError;
use crate::repositories::order_repository::OrderRepository;
use crate::services::{
    work_orders::WorkOrderService,
    orders::OrderService,
    inventory::InventoryService,
    returns::ReturnService,
    shipments::ShipmentService,
    warranties::WarrantyService,
};
use crate::events::EventSender;

// Macro for constructing services with shared resources
macro_rules! init_service {
    ($svc:ident, $db:expr, $sender:expr) => {
        $svc::new($db.clone(), $sender.clone())
    };
}

/// Services layer that encapsulates business logic
#[derive(Debug, Clone)]
pub struct AppServices {
    pub work_orders: WorkOrderService,
    pub orders: OrderService,
    pub inventory: InventoryService,
    pub returns: ReturnService,
    pub shipments: ShipmentService,
    pub warranties: WarrantyService,
}

impl AppServices {
    pub fn new(db_pool: Arc<DatabaseConnection>, event_sender: Arc<EventSender>) -> Self {
        Self {
            work_orders: init_service!(WorkOrderService, db_pool, event_sender),
            orders: init_service!(OrderService, db_pool, event_sender),
            inventory: init_service!(InventoryService, db_pool, event_sender),
            returns: init_service!(ReturnService, db_pool, event_sender),
            shipments: init_service!(ShipmentService, db_pool, event_sender),
            warranties: init_service!(WarrantyService, db_pool, event_sender),
        }
    }
}

/// Application state that will be shared with handlers
#[derive(Debug)]
pub struct AppState {
    db: Arc<DatabaseConnection>,
    config: config::AppConfig,
    order_repository: OrderRepository,
    // Add other repositories as needed
    services: AppServices,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load environment variables from .env file
    dotenv().ok();
    
    // Load configuration from environment variables
    let config = config::load_config()?;

    // Initialize tracing using configuration
    config::init_tracing(&config.log_level);

    info!("Stateset API starting...");
    
    // Connect to the database
    info!("Connecting to database...");
    let db = db::establish_connection(&config.db_url).await
        .map_err(|e| {
            error!("Failed to connect to database: {}", e);
            e
        })?;
    
    // Skip database migrations for now to simplify development
    info!("Skipping database migrations for development...");
    
    // Wrap the database connection in an Arc for sharing
    let db_arc = Arc::new(db);
    
    // Create event channel for domain events
    let (event_tx, event_rx) = tokio::sync::mpsc::channel(100);
    let event_sender = Arc::new(EventSender::new(event_tx));
    
    // Start event processor in the background
    tokio::spawn(async move {
        events::process_events(event_rx).await;
    });
    
    // Initialize repositories
    let order_repository = OrderRepository::new(db_arc.clone());
    
    // Initialize services
    let services = AppServices::new(db_arc.clone(), event_sender.clone());
    
    // Create application state
    let state = Arc::new(AppState { 
        db: db_arc,
        config: config.clone(),
        order_repository,
        services,
    });
    
    // Configure middleware layers
    let middleware = ServiceBuilder::new()
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(Any)
            .allow_headers(Any));
    
    // Set up API routes
    let app = Router::new()
        // Health routes
        .nest("/health", health::health_routes())
        // API v1 routes
        .nest("/api/v1", Router::new()
            .nest("/orders", handlers::orders::orders_routes())
            .nest("/inventory", handlers::inventory::inventory_routes())
            .nest("/returns", handlers::returns::returns_routes())
            .nest("/shipments", handlers::shipments::shipments_routes())
            .nest("/warranties", handlers::warranties::warranties_routes())
            .nest("/work-orders", handlers::work_orders::work_orders_routes())
            // Add other API routes here
        )
        // Configure middleware and state
        .layer(middleware)
        .with_state(state);
    
    // Start the server
    let addr = SocketAddr::from(([0, 0, 0, 0], config.port));
    info!("Listening on {}", addr);
    
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    
    Ok(())
}

