use axum::{
    routing::{get, post},
    Router, Extension,
};
use std::sync::Arc;
use tokio::sync::broadcast;
use slog::{info, o, Drain, Logger};
use dotenv::dotenv;
use opentelemetry::global;
use tower_http::compression::CompressionLayer;
use tower_http::trace::TraceLayer;

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

#[derive(Clone)]
struct AppState {
    config: Arc<AppConfig>,
    db_pool: Arc<db::DbPool>,
    redis_client: Arc<redis::Client>,
    event_sender: broadcast::Sender<events::Event>,
    logger: Logger,
    services: Services,
}

#[derive(Clone)]
struct Services {
    order_service: Arc<services::orders::OrderService>,
    inventory_service: Arc<services::inventory::InventoryService>,
    return_service: Arc<services::returns::ReturnService>,
    warranty_service: Arc<services::warranties::WarrantyService>,
    shipment_service: Arc<services::shipments::ShipmentService>,
    work_order_service: Arc<services::work_orders::WorkOrderService>,
}

#[tokio::main]
async fn main() -> Result<(), AppError> {
    dotenv().ok();
    let config = Arc::new(config::load()?);
    let log = setup_logger(&config);

    info!(log, "Starting StateSet API"; 
        "environment" => &config.environment,
        "version" => env!("CARGO_PKG_VERSION")
    );

    let app_state = build_app_state(&config, &log).await?;

    let schema = Arc::new(graphql::create_schema(
        app_state.services.order_service.clone(),
        app_state.services.inventory_service.clone(),
    ));

    setup_telemetry(&config)?;

    // Spawn event processing
    tokio::spawn(events::process_events(
        app_state.event_sender.subscribe(),
        app_state.services.clone(),
        log.clone(),
    ));

    // Start gRPC server
    #[cfg(feature = "grpc")]
    let grpc_server = grpc_server::start(config.clone(), app_state.services.clone()).await?;

    // Build our application with a route
    let app = Router::new()
        .route("/health", get(health::health_check))
        .nest("/orders", handlers::orders::routes())
        .nest("/inventory", handlers::inventory::routes())
        .nest("/return", handlers::returns::routes())
        .nest("/warranties", handlers::warranties::routes())
        .nest("/shipments", handlers::shipments::routes())
        .nest("/work_orders", handlers::work_orders::routes())
        .nest("/billofmaterials", handlers::billofmaterials::routes())
        .nest("/manufacturing", handlers::manufacturing::routes())
        .nest("/suppliers", handlers::suppliers::routes())
        .route("/proto_endpoint", post(handle_proto_request))
        .layer(Extension(app_state))
        .layer(Extension(schema))
        .layer(TraceLayer::new_for_http())
        .layer(CompressionLayer::new())
        .layer(axum::middleware::from_fn(auth::auth_middleware))
        .layer(axum::middleware::from_fn(rate_limiter::rate_limit_middleware));

    // Run our app with hyper
    let addr = format!("{}:{}", config.host, config.port);
    info!(log, "HTTP server running"; "address" => &addr);
    axum::Server::bind(&addr.parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();

    info!(log, "Shutting down");
    Ok(())
}

fn setup_logger(config: &AppConfig) -> Logger {
    let decorator = slog_term::TermDecorator::new().build();
    let drain = slog_term::FullFormat::new(decorator).build().fuse();
    let drain = slog_async::Async::new(drain).build().fuse();
    let drain = slog::LevelFilter::new(drain, config.log_level.parse().unwrap()).fuse();
    slog::Logger::root(drain, o!())
}

async fn build_app_state(config: &Arc<AppConfig>, log: &Logger) -> Result<AppState, AppError> {
    let db_pool = Arc::new(db::establish_connection(&config.database_url).await?);
    let redis_client = Arc::new(redis::Client::open(&config.redis_url)?);
    let rabbit_conn = message_queue::connect_rabbitmq(&config.rabbitmq_url).await?;
    let (event_sender, _) = broadcast::channel::<events::Event>(100);

    let services = initialize_services(
        db_pool.clone(),
        redis_client.clone(),
        rabbit_conn,
        event_sender.clone(),
        log.clone(),
    ).await?;

    Ok(AppState {
        config: config.clone(),
        db_pool,
        redis_client,
        event_sender,
        logger: log.clone(),
        services,
    })
}

async fn initialize_services(
    db_pool: Arc<db::DbPool>,
    redis_client: Arc<redis::Client>,
    rabbit_conn: lapin::Connection,
    event_sender: broadcast::Sender<events::Event>,
    log: Logger,
) -> Result<Services, AppError> {
    let rate_limiter = Arc::new(rate_limiter::RateLimiter::new(redis_client.clone(), "global", 1000, 60));
    let message_queue = Arc::new(message_queue::RabbitMQ::new(rabbit_conn));
    let circuit_breaker = Arc::new(circuit_breaker::CircuitBreaker::new(5, std::time::Duration::from_secs(60)));

    let inventory_service = Arc::new(services::inventory::InventoryService::new(
        db_pool.clone(),
        event_sender.clone(),
        redis_client.clone(),
        message_queue.clone(),
        circuit_breaker.clone(),
        log.clone(),
    ));

    let order_service = Arc::new(services::orders::OrderService::new(
        db_pool.clone(),
        inventory_service.clone(),
        event_sender.clone(),
        redis_client.clone(),
        message_queue.clone(),
        circuit_breaker.clone(),
        log.clone(),
    ));

    let return_service = Arc::new(services::returns::ReturnService::new(
        db_pool.clone(),
        inventory_service.clone(),
        event_sender.clone(),
        redis_client.clone(),
        message_queue.clone(),
        circuit_breaker.clone(),
        log.clone(),
    ));

    let warranty_service = Arc::new(services::warranties::WarrantyService::new(
        db_pool.clone(),
        event_sender.clone(),
        redis_client.clone(),
        message_queue.clone(),
        circuit_breaker.clone(),
        log.clone(),
    ));

    let shipment_service = Arc::new(services::shipments::ShipmentService::new(
        db_pool.clone(),
        event_sender.clone(),
        redis_client.clone(),
        message_queue.clone(),
        circuit_breaker.clone(),
        log.clone(),
    ));

    let work_order_service = Arc::new(services::work_orders::WorkOrderService::new(
        db_pool.clone(),
        inventory_service.clone(),
        event_sender.clone(),
        redis_client.clone(),
        message_queue.clone(),
        circuit_breaker.clone(),
        log.clone(),
    ));

    Services {
        order_service,
        inventory_service,
        return_service,
        warranty_service,
        shipment_service,
        work_order_service,
    }
}


async fn handle_proto_request(
    Extension(state): Extension<AppState>,
    payload: axum::body::Bytes
) -> Result<axum::response::Response<axum::body::Full<axum::body::Bytes>>, axum::http::StatusCode> {
    // Deserialize the incoming protobuf message
    let request = proto::SomeRequest::decode(payload.as_ref())
        .map_err(|_| axum::http::StatusCode::BAD_REQUEST)?;

    // Process the request (this is where you'd add your business logic)
    let response = proto::SomeResponse {
        // Fill in the response fields based on your logic
        message: format!("Processed request with id: {}", request.id),
    };

    // Serialize the response back to protobuf
    let mut buf = Vec::new();
    response.encode(&mut buf)
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(axum::response::Response::builder()
        .status(axum::http::StatusCode::OK)
        .body(axum::body::Full::from(buf))
        .unwrap())
}

fn setup_telemetry(config: &AppConfig) -> Result<(), AppError> {
    let tracer = opentelemetry_jaeger::new_pipeline()
        .with_service_name("stateset-api")
        .with_endpoint(&config.jaeger_endpoint)
        .install_simple()?;
    global::set_tracer_provider(tracer);
    Ok(())
}