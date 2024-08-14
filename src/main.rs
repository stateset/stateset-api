use actix_web::{web, App, HttpServer, middleware};
use std::sync::Arc;
use tokio::sync::broadcast;
use redis::Client as RedisClient;
use slog::{Drain, Logger, o};
use lapin::{Connection, ConnectionProperties};
use prometheus::{Registry, IntCounterVec};
use opentelemetry::global;
use opentelemetry_jaeger::new_pipeline;
use dotenv::dotenv;
use config::{Config, Environment, File};
use tonic::transport::Server as TonicServer;
use tonic_web::GrpcWebLayer;

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

use services::order_service::OrderServiceImpl;
use services::inventory_service::InventoryServiceImpl;
use services::return_service::ReturnServiceImpl;
use services::warranty_service::WarrantyServiceImpl;
use services::shipment_service::ShipmentServiceImpl;
use services::work_order_service::WorkOrderServiceImpl;
use proto::order_service_server::OrderServiceServer;
use proto::inventory_service_server::InventoryServiceServer;
use proto::return_service_server::ReturnServiceServer;
use proto::warranty_service_server::WarrantyServiceServer;
use proto::shipment_service_server::ShipmentServiceServer;
use proto::work_order_service_server::WorkOrderServiceServer;

#[derive(Clone)]
struct AppState {
    config: Arc<AppConfig>,
    db_pool: Arc<db::DbPool>,
    redis_client: Arc<RedisClient>,
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

#[derive(serde::Deserialize)]
struct AppConfig {
    database_url: String,
    redis_url: String,
    rabbitmq_url: String,
    jaeger_endpoint: String,
    host: String,
    port: u16,
    log_level: String,
    environment: String,
    api_version: String,
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    dotenv().ok();
    let config = Arc::new(load_configuration().expect("Failed to load configuration"));
    let log = setup_logger(&config.log_level);
    slog::info!(log, "Starting Stateset API"; "environment" => &config.environment);

    // Database connection pool
    let db_pool = Arc::new(db::establish_connection(&config.database_url));

    // Redis client setup
    let redis_client = Arc::new(RedisClient::open(&config.redis_url).expect("Failed to connect to Redis"));

    // RabbitMQ connection setup
    let rabbit_conn = Connection::connect(&config.rabbitmq_url, ConnectionProperties::default())
        .await.expect("Failed to connect to RabbitMQ")
        .create_channel()
        .await.expect("Failed to create RabbitMQ channel");

    // Metrics and tracing setup
    let registry = setup_metrics();
    setup_tracing(&config.jaeger_endpoint);

    // Event broadcasting setup
    let (event_sender, _) = broadcast::channel::<events::Event>(100);

    // Initialize services
    let services = initialize_services(db_pool.clone(), redis_client.clone(), rabbit_conn, event_sender.clone(), log.clone()).await;

    // Application state
    let state = AppState {
        config: config.clone(),
        db_pool: db_pool.clone(),
        redis_client: redis_client.clone(),
        event_sender: event_sender.clone(),
        logger: log.clone(),
        services: services.clone()
    };

    // GraphQL schema setup
    let schema = Arc::new(graphql::create_schema(services.order_service.clone(), services.inventory_service.clone()));

    // Spawn event processing in a separate task
    tokio::spawn(events::process_events(event_sender.subscribe(), services.clone(), log.clone()));

    // gRPC server setup
    let grpc_addr = "[::1]:50051".parse().expect("Failed to parse gRPC address");
    let grpc_server = TonicServer::builder()
        .accept_http1(true)
        .layer(GrpcWebLayer::new())
        .add_service(OrderServiceServer::new(OrderServiceImpl::new(db_pool.clone())))
        .add_service(InventoryServiceServer::new(InventoryServiceImpl::new(db_pool.clone())))
        .add_service(ReturnServiceServer::new(ReturnServiceImpl::new(db_pool.clone())))
        .add_service(WarrantyServiceServer::new(WarrantyServiceImpl::new(db_pool.clone())))
        .add_service(ShipmentServiceServer::new(ShipmentServiceImpl::new(db_pool.clone())))
        .add_service(WorkOrderServiceServer::new(WorkOrderServiceImpl::new(db_pool.clone())))
        .serve(grpc_addr);

    // Start the gRPC server in a separate task
    tokio::spawn(async move {
        if let Err(e) = grpc_server.await {
            eprintln!("gRPC server error: {}", e);
        }
    });

    // Start the HTTP server
    let server = HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(state.clone()))
            .app_data(web::Data::new(schema.clone()))
            .wrap(middleware::Logger::default())
            .wrap(tracing::TracingMiddleware::new())
            .wrap(metrics::PrometheusMetrics::new(registry.clone()))
            .wrap(middleware::DefaultHeaders::new()
                .add(("X-Version", env!("CARGO_PKG_VERSION")))
                .add(("X-Environment", state.config.environment.clone())))
            .wrap(middleware::Compress::default())
            .wrap(middleware::NormalizePath::trim())
            .configure(configure_routes)
            .service(web::resource("/health").to(health::health_check))
            .service(web::resource("/proto").to(handle_proto_request))
    })
    .bind(format!("{}:{}", config.host, config.port))?
    .run();

    slog::info!(log, "HTTP server running"; "address" => format!("{}:{}", config.host, config.port));
    slog::info!(log, "gRPC server running"; "address" => grpc_addr.to_string());

    // Graceful shutdown handling
    let srv = server.handle();
    let graceful_shutdown = tokio::spawn(async move {
        srv.stop(true).await;
    });

    tokio::select! {
        _ = server => {},
        _ = graceful_shutdown => {},
    }

    slog::info!(log, "Shutting down");
    Ok(())
}

fn load_configuration() -> Result<AppConfig, config::ConfigError> {
    let mut config = Config::default();
    config.merge(File::with_name("config/default"))?;
    config.merge(Environment::with_prefix("APP"))?;
    config.try_into()
}

fn setup_logger(log_level: &str) -> Logger {
    let decorator = slog_term::TermDecorator::new().build();
    let drain = slog_term::FullFormat::new(decorator).build().fuse();
    let drain = slog_async::Async::new(drain).build().fuse();
    let drain = slog::LevelFilter::new(drain, log_level.parse().unwrap()).fuse();
    slog::Logger::root(drain, o!())
}

fn setup_metrics() -> Registry {
    let registry = Registry::new();
    let http_requests_total = IntCounterVec::new(
        prometheus::opts!("http_requests_total", "Total number of HTTP requests"),
        &["method", "path", "status"]
    ).expect("Failed to create http_requests_total metric");
    registry.register(Box::new(http_requests_total)).expect("Failed to register http_requests_total metric");
    registry
}

fn setup_tracing(jaeger_endpoint: &str) {
    let tracer = new_pipeline()
        .with_service_name("stateset-api")
        .with_endpoint(jaeger_endpoint)
        .install_simple()
        .expect("Failed to install Jaeger tracer");
    global::set_tracer_provider(tracer);
}

async fn initialize_services(
    db_pool: Arc<db::DbPool>,
    redis_client: Arc<RedisClient>,
    rabbit_channel: lapin::Channel,
    event_sender: broadcast::Sender<events::Event>,
    log: Logger,
) -> Services {
    let rate_limiter = Arc::new(rate_limiter::RateLimiter::new(redis_client.clone(), "global", 1000, 60));
    let message_queue = Arc::new(message_queue::RabbitMQ::new(rabbit_channel));
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
    state: web::Data<AppState>,
    payload: web::Bytes
) -> Result<web::Bytes, actix_web::Error> {
    // Deserialize the incoming protobuf message
    let request = proto::SomeRequest::decode(payload.as_ref())
        .map_err(|e| actix_web::error::ErrorBadRequest(e))?;

    // Process the request (this is where you'd add your business logic)
    let response = proto::SomeResponse {
        // Fill in the response fields based on your logic
        message: format!("Processed request with id: {}", request.id),
    };

    // Serialize the response back to protobuf
    let mut buf = Vec::new();
    response.encode(&mut buf)
        .map_err(|e| actix_web::error::ErrorInternalServerError(e))?;

    Ok(web::Bytes::from(buf))
}

fn configure_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/orders")
            .wrap(auth::AuthMiddleware::new(vec!["user", "admin"]))
            .wrap(rate_limiter::RateLimitMiddleware::new(100, 60))
            .configure(handlers::orders::configure_routes)
    );

    cfg.service(
        web::scope("/inventory")
            .wrap(auth::AuthMiddleware::new(vec!["user", "admin"]))
            .wrap(rate_limiter::RateLimitMiddleware::new(100, 60))
            .configure(handlers::inventory::configure_routes)
    );

    cfg.service(
        web::scope("/returns")
            .wrap(auth::AuthMiddleware::new(vec!["user", "admin"]))
            .wrap(rate_limiter::RateLimitMiddleware::new(100, 60))
            .configure(handlers::returns::configure_routes)
    );

    cfg.service(
        web::scope("/warranties")
            .wrap(auth::AuthMiddleware::new(vec!["user", "admin"]))
            .wrap(rate_limiter::RateLimitMiddleware::new(100, 60))
            .configure(handlers::warranties::configure_routes)
    );

    cfg.service(
        web::scope("/shipments")
            .wrap(auth::AuthMiddleware::new(vec!["user", "admin"]))
            .wrap(rate_limiter::RateLimitMiddleware::new(100, 60))
            .configure(handlers::shipments::configure_routes)
    );

    cfg.service(
        web::scope("/work_orders")
            .wrap(auth::AuthMiddleware::new(vec!["user", "admin"]))
            .wrap(rate_limiter::RateLimitMiddleware::new(100, 60))
            .configure(handlers::work_orders::configure_routes)
    );

    cfg.service(
        web::scope("/billofmaterials")
            .wrap(auth::AuthMiddleware::new(vec!["user", "admin"]))
            .wrap(rate_limiter::RateLimitMiddleware::new(100, 60))
            .configure(handlers::billofmaterials::configure_routes)
    );

    cfg.service(
        web::scope("/manufacturing")
            .wrap(auth::AuthMiddleware::new(vec!["user", "admin"]))
            .wrap(rate_limiter::RateLimitMiddleware::new(100, 60))
            .configure(handlers::manufacturing::configure_routes)
    );

    cfg.service(
        web::scope("/suppliers")
            .wrap(auth::AuthMiddleware::new(vec!["user", "admin"]))
            .wrap(rate_limiter::RateLimitMiddleware::new(100, 60))
            .configure(handlers::suppliers::configure_routes)
    );

    cfg.service(
        web::resource("/proto_endpoint")
            .wrap(auth::AuthMiddleware::new(vec!["user", "admin"]))
            .wrap(rate_limiter::RateLimitMiddleware::new(100, 60))
            .route(web::post().to(handle_proto_request))
    );
}