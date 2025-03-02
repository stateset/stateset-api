use axum::{
    routing::{get, post},
    Router, Extension,
};
use std::sync::Arc;
use tokio::sync::broadcast;
use slog::{info, o, Drain, Logger};
use dotenv::dotenv;
use opentelemetry::global;
use tower_http::{compression::CompressionLayer, trace::TraceLayer};
use anyhow::{Result, Context};

mod config;
mod services;
mod models;
mod handlers;
mod events;
mod commands;
mod queries;
mod errors;
mod logging;
mod cache;
mod rate_limiter;
mod message_queue;
mod circuit_breaker;
mod tracing;
mod health;
mod db;
mod proto;
mod auth;
mod grpc_server;

use config::AppConfig;
use errors::AppError;

// Constants
const EVENT_CHANNEL_CAPACITY: usize = 100;
const DEFAULT_RATE_LIMIT: usize = 1000;
const RATE_LIMIT_WINDOW_SECS: u64 = 60;

/// Application State holding shared resources and services
#[derive(Clone)]
struct AppState {
    config: Arc<AppConfig>,
    db_pool: Arc<db::DbPool>,
    redis_client: Arc<redis::Client>,
    event_sender: broadcast::Sender<events::Event>,
    logger: Logger,
    services: Arc<Services>,
}

/// Grouped Services for better organization
#[derive(Clone)]
struct Services {
    // Core business services
    orders: Arc<services::orders::OrderService>,
    inventory: Arc<services::inventory::InventoryService>,
    returns: Arc<services::returns::ReturnService>,
    warranties: Arc<services::warranties::WarrantyService>,
    shipments: Arc<services::shipments::ShipmentService>,
    
    // Manufacturing and Supply Chain
    work_orders: Arc<services::work_orders::WorkOrderService>,
    bill_of_materials: Arc<services::billofmaterials::BillOfMaterialsService>,
    suppliers: Arc<services::suppliers::SupplierService>,
    procurement: Arc<services::procurement::ProcurementService>,
    
    // Customer Management
    customers: Arc<services::customers::CustomerService>,
    leads: Arc<services::leads::LeadsService>,
    accounts: Arc<services::accounts::AccountService>,
    
    // Financial Services
    invoicing: Arc<services::invoicing::InvoicingService>,
    payments: Arc<services::payments::PaymentService>,
    accounting: Arc<services::accounting::AccountingService>,
    
    // Analytics and Reporting
    business_intelligence: Arc<services::business_intelligence::BusinessIntelligenceService>,
    forecasting: Arc<services::forecasting::ForecastingService>,
    reports: Arc<services::reports::ReportService>,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize environment and configuration
    dotenv().ok();
    let config = Arc::new(config::load().context("Failed to load configuration")?);
    let logger = setup_logger(&config)?;

    // Log startup information
    info!(logger, "Starting StateSet API";
        "environment" => &config.environment,
        "version" => env!("CARGO_PKG_VERSION")
    );

    // Build application state
    let app_state = build_app_state(&config, &logger)
        .await
        .context("Failed to build application state")?;

    // Setup telemetry
    setup_telemetry(&config).context("Failed to setup telemetry")?;

    // Spawn background tasks
    spawn_background_tasks(app_state.clone(), logger.clone());

    // Setup HTTP server
    let app = setup_router(app_state)
        .layer(axum::middleware::from_fn(auth::auth_middleware))
        .layer(axum::middleware::from_fn(rate_limiter::rate_limit_middleware));

    // Start server
    let addr = format!("{}:{}", config.host, config.port);
    info!(logger, "StateSet API server running"; "address" => &addr);

    axum::Server::bind(&addr.parse()?)
        .serve(app.into_make_service())
        .await
        .context("Server failed to start")?;

    info!(logger, "Shutting down");
    Ok(())
}

fn setup_router(state: AppState) -> Router {
    let schema = Arc::new(graphql::create_schema(state.services.clone()));
    
    Router::new()
        .route("/health", get(handlers::health::health_check))
        .nest("/api", api_routes())
        .route("/proto_endpoint", post(handle_proto_request))
        .layer(Extension(state))
        .layer(Extension(schema))
        .layer(TraceLayer::new_for_http())
        .layer(CompressionLayer::new())
}

fn api_routes() -> Router {
    Router::new()
        .nest("/orders", handlers::orders::routes())
        .nest("/inventory", handlers::inventory::routes())
        .nest("/returns", handlers::returns::routes())
        .nest("/warranties", handlers::warranties::routes())
        .nest("/shipments", handlers::shipments::routes())
}

fn setup_logger(config: &AppConfig) -> Result<Logger> {
    let decorator = slog_term::TermDecorator::new().build();
    let drain = slog_term::FullFormat::new(decorator).build().fuse();
    let drain = slog_async::Async::new(drain).build().fuse();
    let drain = slog::LevelFilter::new(drain, config.log_level.parse()?).fuse();
    Ok(slog::Logger::root(drain, o!()))
}

async fn build_app_state(config: &Arc<AppConfig>, logger: &Logger) -> Result<AppState> {
    let db_pool = Arc::new(db::establish_connection(&config.database_url).await?);
    let redis_client = Arc::new(redis::Client::open(&config.redis_url)?);
    let rabbit_conn = message_queue::connect_rabbitmq(&config.rabbitmq_url).await?;
    let (event_sender, _) = broadcast::channel::<events::Event>(EVENT_CHANNEL_CAPACITY);

    let services = Arc::new(initialize_services(
        db_pool.clone(),
        redis_client.clone(),
        rabbit_conn,
        event_sender.clone(),
        logger.clone(),
    ).await?);

    Ok(AppState {
        config: config.clone(),
        db_pool,
        redis_client,
        event_sender,
        logger: logger.clone(),
        services,
    })
}

async fn initialize_services(
    db_pool: Arc<db::DbPool>,
    redis_client: Arc<redis::Client>,
    rabbit_conn: lapin::Connection,
    event_sender: broadcast::Sender<events::Event>,
    logger: Logger,
) -> Result<Services> {
    let message_queue = Arc::new(message_queue::RabbitMQ::new(rabbit_conn));
    let circuit_breaker = Arc::new(circuit_breaker::CircuitBreaker::new(5, std::time::Duration::from_secs(60)));

    macro_rules! new_service {
        ($module:ident) => {
            Arc::new(services::$module::new(
                db_pool.clone(),
                event_sender.clone(),
                redis_client.clone(),
                message_queue.clone(),
                circuit_breaker.clone(),
                logger.clone(),
            ))
        };
    }

    Ok(Services {
        orders: new_service!(orders),
        inventory: new_service!(inventory),
        returns: new_service!(returns),
        warranties: new_service!(warranties),
        shipments: new_service!(shipments),
        work_orders: new_service!(work_orders),
        bill_of_materials: new_service!(billofmaterials),
        suppliers: new_service!(suppliers),
        procurement: new_service!(procurement),
        customers: new_service!(customers),
        leads: new_service!(leads),
        accounts: new_service!(accounts),
        invoicing: new_service!(invoicing),
        payments: new_service!(payments),
        accounting: new_service!(accounting),
        business_intelligence: new_service!(business_intelligence),
        forecasting: new_service!(forecasting),
        reports: new_service!(reports),
    })
}

fn spawn_background_tasks(state: AppState, logger: Logger) {
    tokio::spawn(events::process_events(
        state.event_sender.subscribe(),
        state.services.clone(),
        logger.clone(),
    ));

    #[cfg(feature = "grpc")]
    tokio::spawn(grpc_server::start(
        state.config.clone(),
        state.services.clone(),
    ));
}

async fn handle_proto_request(
    Extension(state): Extension<AppState>,
    payload: axum::body::Bytes,
) -> Result<axum::response::Response, axum::http::StatusCode> {
    let request = proto::SomeRequest::decode(payload.as_ref())
        .map_err(|_| axum::http::StatusCode::BAD_REQUEST)?;

    let response = proto::SomeResponse {
        message: format!("Processed request with id: {}", request.id),
    };

    let mut buf = Vec::new();
    response.encode(&mut buf)
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(axum::response::Response::new(axum::body::Body::from(buf)))
}

fn setup_telemetry(config: &AppConfig) -> Result<()> {
    let tracer = opentelemetry_jaeger::new_pipeline()
        .with_service_name("stateset-api")
        .with_endpoint(&config.jaeger_endpoint)
        .install_simple()?;
    global::set_tracer_provider(tracer);
    Ok(())
}