use actix_web::{web, App, HttpServer, middleware};
use std::sync::Arc;
use tokio::sync::broadcast;
use slog::{info, o, Drain, Logger};
use dotenv::dotenv;
use opentelemetry::global;

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

#[actix_web::main]
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

    // Start HTTP server
    let server = HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(app_state.clone()))
            .app_data(web::Data::new(schema.clone()))
            .wrap(middleware::Logger::default())
            .wrap(tracing::TracingMiddleware::new())
            .wrap(metrics::PrometheusMetrics::new())
            .wrap(middleware::Compress::default())
            .wrap(middleware::NormalizePath::trim())
            .configure(configure_routes)
            .service(web::resource("/health").to(health::health_check))
    })
    .bind(format!("{}:{}", config.host, config.port))?
    .run();

    info!(log, "HTTP server running"; 
        "address" => format!("{}:{}", config.host, config.port)
    );

    // Graceful shutdown handling
    let graceful_shutdown = shutdown_signal();

    tokio::select! {
        _ = server => {},
        _ = graceful_shutdown => {
            info!(log, "Initiating graceful shutdown");
        },
    }

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

fn setup_telemetry(config: &AppConfig) -> Result<(), AppError> {
    let tracer = opentelemetry_jaeger::new_pipeline()
        .with_service_name("stateset-api")
        .with_endpoint(&config.jaeger_endpoint)
        .install_simple()?;
    global::set_tracer_provider(tracer);
    Ok(())
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
        web::scope("/return")
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

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
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
}