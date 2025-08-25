use std::sync::Arc;
use std::time::Duration;

use axum::{Router, http::StatusCode, Json};
use serde_json::json;
use tokio::signal;
use tower::ServiceBuilder;
use tower_http::{
    cors::CorsLayer,
    timeout::TimeoutLayer,
    trace::TraceLayer,
    compression::CompressionLayer,
};
use tracing::instrument;

use stateset_api::{
    api::StateSetApi,
    api_v1_routes,
    config,
    db,
    events::{process_events, EventSender},
    health,
    proto::*,
    services,
    AppState,
    openapi,
    rate_limiter::{RateLimitConfig, RateLimitLayer, PathPolicy},
    tracing::RequestLoggerLayer,
    versioning,
};
use stateset_api::auth::AuthRouterExt;
use tonic::transport::Server;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing (early); will reconfigure after loading config
    let _ = tracing_subscriber::fmt::try_init();

    tracing::info!("Starting StateSet API server...");

    // Load configuration
    let config = config::load_config()?;
    // Re-init tracing with configured log level and format
    stateset_api::config::init_tracing(&config.log_level, config.log_json);
    tracing::info!("Configuration loaded successfully");

    // Initialize database connection (with pool tuning)
    let db_arc = Arc::new(db::establish_connection_from_app_config(&config).await?);
    tracing::info!("Database connection established");

    // Run database migrations in development by default, or when explicitly enabled
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

    // Create database access wrapper
    let db_access = Arc::new(db::DatabaseAccess::new(db_arc.clone()));
    
    // Create inventory service
    let inventory_service = services::inventory::InventoryService::new(
        db_arc.clone(),
        event_sender.clone(),
    );
    
    // Initialize auth service and attach as an Extension for middleware to use
    let auth_config = stateset_api::auth::AuthConfig::new(
        config.jwt_secret.clone(),
        "stateset-api".to_string(),
        "stateset-auth".to_string(),
        Duration::from_secs(config.jwt_expiration as u64),
        Duration::from_secs(config.refresh_token_expiration as u64),
        "sk_".to_string(),
    );
    let auth_service = std::sync::Arc::new(stateset_api::auth::AuthService::new(auth_config, db_arc.clone()));

    // Initialize Redis client for notifications and other features
    let redis_client = std::sync::Arc::new(redis::Client::open(config.redis_url.clone())?);

    // Create application state for HTTP API
    let state = AppState {
        db: db_arc.clone(),
        config: config.clone(),
        event_sender: event_sender.clone(),
        inventory_service,
        services: stateset_api::handlers::AppServices::new(
            db_arc.clone(),
            std::sync::Arc::new(event_sender.clone()),
            redis_client.clone(),
            auth_service.clone(),
        ),
        redis: redis_client.clone(),
    };

    // Seed demo data in development for smoother local testing
    if !state.config.is_production() {
        if let Err(e) = state.services.order.ensure_demo_order().await {
            tracing::warn!("Demo order seeding failed: {}", e);
        }
    }

    // Create StateSet API for gRPC with shared event sender
    let stateset_api = StateSetApi::with_event_sender(db_access, db_arc.clone(), event_sender.clone());

    // Create enhanced API routes
    let api_routes = api_v1_routes().with_state(state.clone());

    // Parse path-based rate limit policies from config (prefix:limit:window_secs, comma-separated)
    let mut path_policies: Vec<PathPolicy> = Vec::new();
    let mut api_key_policies: std::collections::HashMap<String, (u32, Duration)> = std::collections::HashMap::new();
    let mut user_policies: std::collections::HashMap<String, (u32, Duration)> = std::collections::HashMap::new();
    if let Some(spec) = &config.rate_limit_path_policies {
        for part in spec.split(',') {
            let segs: Vec<&str> = part.split(':').collect();
            if segs.len() == 3 {
                if let (Ok(limit), Ok(win)) = (segs[1].trim().parse::<u32>(), segs[2].trim().parse::<u64>()) {
                    path_policies.push(PathPolicy {
                        prefix: segs[0].trim().to_string(),
                        requests_per_window: limit,
                        window_duration: Duration::from_secs(win),
                    });
                }
            }
        }
    }
    if let Some(spec) = &config.rate_limit_api_key_policies {
        for part in spec.split(',') {
            let segs: Vec<&str> = part.split(':').collect();
            if segs.len() == 3 {
                if let (Ok(limit), Ok(win)) = (segs[1].trim().parse::<u32>(), segs[2].trim().parse::<u64>()) {
                    api_key_policies.insert(segs[0].trim().to_string(), (limit, Duration::from_secs(win)));
                }
            }
        }
    }
    if let Some(spec) = &config.rate_limit_user_policies {
        for part in spec.split(',') {
            let segs: Vec<&str> = part.split(':').collect();
            if segs.len() == 3 {
                if let (Ok(limit), Ok(win)) = (segs[1].trim().parse::<u32>(), segs[2].trim().parse::<u64>()) {
                    user_policies.insert(segs[0].trim().to_string(), (limit, Duration::from_secs(win)));
                }
            }
        }
    }

    // Build API router with heavy middleware stack only for API paths
    let api_router = Router::new()
        .nest("/api/v1", api_routes)
        // Additional domain routers using their own state types
        // .nest("/api/v1/customers", stateset_api::handlers::customers::customer_routes().with_state(db_arc.clone()).with_permission("customers:access"))
        // .nest("/api/v1/purchase-orders", stateset_api::handlers::purchase_orders::purchase_order_routes().with_state(std::sync::Arc::new(state.clone())).with_permission("purchase_orders:access"))
        // .nest("/api/v1/suppliers", stateset_api::handlers::suppliers::supplier_routes().with_state(std::sync::Arc::new(state.clone())).with_permission("suppliers:access"))
        .nest(
            "/api/v1/users",
            stateset_api::handlers::users::user_routes()
                .with_state(std::sync::Arc::new(state.clone()))
                .with_permission("users:access"),
        )
        // .nest("/api/v1/bom", stateset_api::handlers::bom::bom_routes().with_state(std::sync::Arc::new(state.clone())).with_permission("bom:access"))
        // .nest("/api/v1/cash-sales", stateset_api::handlers::cash_sales::cash_sale_routes().with_state(std::sync::Arc::new(state.clone())).with_permission("cash_sales:access"))
        // .nest("/api/v1/reports", stateset_api::handlers::reports::report_routes().with_state(std::sync::Arc::new(state.clone())).with_permission("reports:access"))
        .nest(
            "/api/v1/commerce/products",
            stateset_api::handlers::commerce::products::products_routes()
                .with_state(std::sync::Arc::new(state.clone()))
                .with_permission("commerce:products"),
        )
        .nest(
            "/api/v1/commerce/carts",
            stateset_api::handlers::commerce::carts::carts_routes()
                .with_state(std::sync::Arc::new(state.clone()))
                .with_permission("commerce:carts"),
        )
        .nest(
            "/api/v1/commerce/checkout",
            stateset_api::handlers::commerce::checkout::checkout_routes()
                .with_state(std::sync::Arc::new(state.clone()))
                .with_permission("commerce:checkout"),
        )
        .nest(
            "/api/v1/commerce/customers",
            stateset_api::handlers::commerce::customers::customers_routes()
                .with_state(std::sync::Arc::new(state.clone()))
                .with_permission("commerce:customers"),
        )
        .nest(
            "/api/v1/notifications",
            stateset_api::handlers::notifications::notification_routes()
                .with_state((db_arc.clone(), redis_client.clone()))
                .with_permission("notifications:access"),
        )
        // .nest("/api/v1/asn", stateset_api::handlers::asn::asn_routes().with_state(db_arc.clone()).with_permission("asn:access"))
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
                .layer(TimeoutLayer::new(Duration::from_secs(30)))
                .layer(cross_origin_layer(&config))
                .layer(CompressionLayer::new())
        )
        .layer(axum::Extension(auth_service.clone()))
        .layer(axum::extract::DefaultBodyLimit::max(2 * 1024 * 1024))
        .layer(axum::Extension(stateset_api::middleware_helpers::IdempotencyStore::new()))
        .layer(axum::middleware::from_fn(stateset_api::middleware_helpers::request_id_middleware))
        .layer(axum::middleware::from_fn(stateset_api::middleware_helpers::security_headers::security_headers_middleware))
        .layer(axum::middleware::from_fn(stateset_api::middleware_helpers::sanitize_middleware))
        .layer(RequestLoggerLayer::new())
        .layer(RateLimitLayer::new(RateLimitConfig {
            requests_per_window: config.rate_limit_requests_per_window,
            window_duration: Duration::from_secs(config.rate_limit_window_seconds),
            burst_limit: None,
            enable_headers: config.rate_limit_enable_headers,
        })
        .with_policies(path_policies)
        .with_api_key_policies(api_key_policies)
        .with_user_policies(user_policies))
        .layer(axum::middleware::from_fn(stateset_api::middleware_helpers::idempotency_middleware))
        .layer(axum::middleware::from_fn(stateset_api::versioning::api_version_middleware));

    let app = Router::new()
        // Health and metrics: minimal middleware, no rate limit
        .nest("/health", health::health_routes_with_state(db_arc.clone()))
        // Common health/readiness aliases
        .route("/readyz", axum::routing::get(|| async { axum::response::Redirect::to("/health/ready") }))
        .route("/livez", axum::routing::get(|| async { axum::response::Redirect::to("/health/live") }))
        .route("/-/healthz", axum::routing::get(|| async { axum::response::Redirect::to("/health") }))
        .route("/-/ready", axum::routing::get(|| async { axum::response::Redirect::to("/health/ready") }))
        .route("/-/live", axum::routing::get(|| async { axum::response::Redirect::to("/health/live") }))
        // Version info alias at root
        .route("/version", axum::routing::get(health::version_info))
        // Root API info
        .route("/", axum::routing::get(api_root_info))
        // Keep alias for historical link, redirect to /docs
        .route("/swagger-ui", axum::routing::get(|| async { axum::response::Redirect::to("/docs") }))
        .route("/metrics", axum::routing::get(metrics_endpoint))
        .route("/metrics/json", axum::routing::get(metrics_json_endpoint))
        // API versions info and docs
        .nest("/api/versions", versioning::api_versions_routes())
        .merge(openapi::swagger_routes())
        .nest("/api-docs", openapi::create_docs_routes())
        // Auth endpoints (login/refresh) use their own state (AuthService)
        .nest("/api/v1/auth", stateset_api::auth::auth_routes().with_state(auth_service.clone()))
        // Mount API router
        .merge(api_router)
        // Fallback 404 JSON
        .fallback(fallback_handler);

    // Configure server addresses
    let http_addr = format!("{}:{}", config.host, config.port);
    let grpc_port = config.grpc_port.unwrap_or(config.port + 1);
    let grpc_addr = format!("{}:{}", config.host, grpc_port).parse()?;
    
    // Start HTTP server
    let listener = tokio::net::TcpListener::bind(&http_addr).await?;
    tracing::info!("🚀 StateSet HTTP API server listening on http://{}", http_addr);
    
    // Start gRPC server
    tracing::info!("🚀 StateSet gRPC API server listening on grpc://{}", grpc_addr);

    let grpc_server = Server::builder()
        .add_service(order::order_service_server::OrderServiceServer::new(stateset_api.clone()))
        .add_service(inventory::inventory_service_server::InventoryServiceServer::new(stateset_api.clone()))
        .add_service(return_order::return_service_server::ReturnServiceServer::new(stateset_api.clone()))
        .add_service(warranty::warranty_service_server::WarrantyServiceServer::new(stateset_api.clone()))
        .add_service(shipment::shipment_service_server::ShipmentServiceServer::new(stateset_api.clone()))
        .add_service(work_order::work_order_service_server::WorkOrderServiceServer::new(stateset_api))
        .serve_with_shutdown(grpc_addr, shutdown_signal());

    // Run both servers concurrently
    let http_server = axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal());

    // Start both servers with proper error handling
    let result = tokio::select! {
        res = http_server => {
            tracing::error!("HTTP server stopped: {:?}", res);
            res.map_err(anyhow::Error::from)
        }
        res = grpc_server => {
            tracing::error!("gRPC server stopped: {:?}", res);
            res.map_err(anyhow::Error::from)
        }
        _ = shutdown_signal() => {
            tracing::info!("Graceful shutdown initiated");
            Ok(())
        }
    };

    // Clean up
    event_processor_handle.abort();
    let _ = event_processor_handle.await;
    tracing::info!("✅ StateSet API server shutdown complete");

    result
}

// Build CORS layer based on AppConfig
fn cross_origin_layer(cfg: &stateset_api::config::AppConfig) -> CorsLayer {
    use tower_http::cors::{AllowOrigin, CorsLayer as InnerCors};
    if let Some(list) = &cfg.cors_allowed_origins {
        let origins: Vec<_> = list
            .split(',')
            .filter_map(|o| o.trim().parse().ok())
            .collect();
        if !origins.is_empty() {
            let mut layer = InnerCors::new().allow_origin(AllowOrigin::list(origins));
            if cfg.cors_allow_credentials {
                layer = layer.allow_credentials(true);
            }
            return layer;
        }
    }
    // Default: permissive during development
    if !cfg.is_production() {
        InnerCors::permissive()
    } else {
        // In production with no list provided, deny all by default.
        InnerCors::new().allow_origin(AllowOrigin::list(Vec::<http::HeaderValue>::new()))
    }
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

#[instrument]
async fn metrics_endpoint() -> Result<String, (StatusCode, String)> {
    stateset_api::metrics::metrics_handler()
        .await
        .map_err(|e| {
            tracing::error!("Metrics handler error: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, format!("Metrics export failed: {}", e))
        })
}

#[instrument]
async fn metrics_json_endpoint() -> Result<axum::Json<serde_json::Value>, (StatusCode, String)> {
    stateset_api::metrics::metrics_json_handler()
        .await
        .map(axum::Json)
        .map_err(|e| {
            tracing::error!("Metrics JSON handler error: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, format!("Metrics JSON export failed: {}", e))
        })
}

#[instrument]
async fn fallback_handler() -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::NOT_FOUND,
        Json(json!({
            "error": "not_found",
            "message": "The requested resource was not found",
            "status": 404,
            "timestamp": chrono::Utc::now().to_rfc3339()
        }))
    )
}

#[instrument]
async fn api_root_info() -> Json<serde_json::Value> {
    Json(json!({
        "name": "stateset-api",
        "version": env!("CARGO_PKG_VERSION"),
        "status": "up",
        "docs": {
            "openapi_index": "/api-docs",
            "swagger_ui": "/docs",
        },
        "endpoints": {
            "health": "/health",
            "health_aliases": ["/readyz", "/livez", "/-/healthz"],
            "metrics": "/metrics",
            "api_versions": "/api/versions",
            "version": "/version",
        },
        "timestamp": chrono::Utc::now().to_rfc3339(),
    }))
}
