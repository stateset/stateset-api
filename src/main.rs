use actix_web::{web, App, HttpServer, middleware};
use std::sync::Arc;
use tokio::sync::broadcast;
use redis::Client as RedisClient;
use slog::{Drain, Logger};
use lapin::{Connection, ConnectionProperties};
use prometheus::{Registry, IntCounterVec};
use opentelemetry::global;
use opentelemetry_jaeger::new_pipeline;
use dotenv::dotenv;
use config::{Config, Environment, File};
use futures::future::join_all;

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
mod metrics;
mod tracing;
mod graphql;
mod feature_flags;
mod ml;
mod health;

#[derive(Clone)]
struct AppState {
    config: Arc<AppConfig>,
    db_pool: Arc<DbPool>,
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
    let config = Arc::new(load_configuration()?);
    let log = setup_logger(&config.log_level);
    slog::info!(log, "Starting Stateset API"; "environment" => &config.environment);

    // Database connection pool
    let db_pool = Arc::new(db::establish_connection(&config.database_url));

    // Redis client setup
    let redis_client = Arc::new(RedisClient::open(&config.redis_url)?);

    // RabbitMQ connection setup
    let rabbit_conn = Connection::connect(&config.rabbitmq_url, ConnectionProperties::default())
        .await?
        .create_channel()
        .await?;

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

    // Start the HTTP server
    let server = HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(state.clone()))
            .app_data(web::Data::new(schema.clone()))
            .wrap(middleware::Logger::default())
            .wrap(tracing::TracingMiddleware::new())
            .wrap(metrics::PrometheusMetrics::new(registry.clone()))
            .wrap(middleware::DefaultHeaders::new()
                .header("X-Version", env!("CARGO_PKG_VERSION"))
                .header("X-Environment", &state.config.environment))
            .wrap(middleware::Compress::default())
            .wrap(middleware::NormalizePath::new(middleware::TrailingSlash::Trim))
            .configure(configure_routes)
            .service(web::resource("/health").to(health::health_check))
    })
    .bind(format!("{}:{}", config.host, config.port))?
    .run();

    slog::info!(log, "Server running"; "address" => format!("{}:{}", config.host, config.port));

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
    let drain = slog::LevelFilter::new(drain, slog::Level::from_str(log_level).unwrap()).fuse();
    slog::Logger::root(drain, slog::o!())
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
    db_pool: Arc<DbPool>,
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
}
