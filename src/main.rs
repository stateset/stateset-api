use std::{net::SocketAddr, sync::Arc, time::Duration};

use anyhow::Context;
use axum::http::StatusCode;
use axum::{routing::get, Router};
use http::HeaderValue;
use tokio::{signal, sync::mpsc};
use tower_http::{
    compression::CompressionLayer,
    cors::{Any, CorsLayer},
};
use tracing::{error, info, warn};

use stateset_api as api;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cfg = api::config::load_config()?;
    api::config::init_tracing(cfg.log_level(), cfg.log_json);

    // Init DB
    let db_pool = api::db::establish_connection_from_app_config(&cfg).await?;
    if cfg.auto_migrate {
        api::db::run_migrations(&db_pool).await.map_err(|e| {
            error!("Failed running migrations: {}", e);
            e
        })?;
    }

    // Init Redis client (construction only; connection checked in health)
    let redis_client = Arc::new(redis::Client::open(cfg.redis_url.clone())?);

    // Build services
    let db_arc = Arc::new(db_pool);
    // Init events
    let (event_tx, event_rx) = mpsc::channel(1024);
    let event_sender = api::events::EventSender::new(event_tx);

    // Initialize Agentic Commerce webhook service
    let webhook_service = cfg.agentic_commerce_webhook_secret.clone().map(|secret| {
        info!("Agentic Commerce webhook delivery enabled");
        Arc::new(api::webhooks::AgenticCommerceWebhookService::new(Some(
            secret,
        )))
    });
    let webhook_url = cfg.agentic_commerce_webhook_url.clone();

    if webhook_url.is_some() {
        info!("Agentic Commerce webhook URL configured: {:?}", webhook_url);
    } else {
        info!("Agentic Commerce webhook URL not configured; outbound webhooks disabled");
    }

    // Spawn event processor with webhook support
    tokio::spawn(api::events::process_events(
        event_rx,
        webhook_service,
        webhook_url,
    ));

    // Start outbox worker (best-effort, no-op if table missing)
    api::events::outbox::start_worker(db_arc.clone(), event_sender.clone()).await;
    let inventory_service =
        api::services::inventory::InventoryService::new(db_arc.clone(), event_sender.clone());

    // Auth service for handlers/services requiring it
    let auth_cfg = api::auth::AuthConfig::new(
        cfg.jwt_secret.clone(),
        cfg.auth_issuer.clone(),
        cfg.auth_audience.clone(),
        Duration::from_secs(cfg.jwt_expiration as u64),
        Duration::from_secs(cfg.refresh_token_expiration as u64),
        cfg.api_key_prefix.clone(),
    )
    .context("failed to create auth config")?;
    let auth_service = Arc::new(api::auth::AuthService::new(auth_cfg, db_arc.clone()));

    // Prepare shared queue and logger infrastructure
    let base_logger = api::logging::setup_logger(api::logging::LoggerConfig::default());
    let message_queue: Arc<dyn api::message_queue::MessageQueue> =
        match cfg.message_queue_backend.to_ascii_lowercase().as_str() {
            "redis" => match api::message_queue::RedisMessageQueue::new(
                redis_client.clone(),
                cfg.message_queue_namespace.clone(),
                Duration::from_secs(cfg.message_queue_block_timeout_secs),
            )
            .await
            {
                Ok(queue) => Arc::new(queue),
                Err(err) => {
                    error!(
                        "Failed to initialize Redis message queue (falling back to in-memory): {}",
                        err
                    );
                    Arc::new(api::message_queue::InMemoryMessageQueue::new())
                }
            },
            _ => Arc::new(api::message_queue::InMemoryMessageQueue::new()),
        };

    // Aggregate app services used by HTTP handlers
    let services = api::handlers::AppServices::new(
        db_arc.clone(),
        Arc::new(event_sender.clone()),
        redis_client.clone(),
        auth_service.clone(),
        message_queue,
        base_logger,
    );

    // Compose shared app state
    let app_state = api::AppState {
        db: db_arc.clone(),
        config: cfg.clone(),
        event_sender,
        inventory_service,
        services,
        redis: redis_client.clone(),
    };

    // Build CORS layer from config
    let configured_origins: Option<Vec<HeaderValue>> = cfg
        .cors_allowed_origins
        .as_ref()
        .map(|raw| {
            raw.split(',')
                .filter_map(|origin| {
                    let trimmed = origin.trim();
                    if trimmed.is_empty() {
                        None
                    } else {
                        HeaderValue::from_str(trimmed).ok()
                    }
                })
                .collect::<Vec<_>>()
        })
        .filter(|origins| !origins.is_empty());

    let cors_layer = if let Some(origins) = configured_origins {
        CorsLayer::new()
            .allow_origin(origins)
            .allow_methods(Any)
            .allow_headers(Any)
            .allow_credentials(cfg.cors_allow_credentials)
    } else if cfg.should_allow_permissive_cors() {
        info!(
            "Using permissive CORS because explicit origins were not configured ({})",
            if cfg.is_development() {
                "development environment"
            } else {
                "explicit override enabled"
            }
        );
        CorsLayer::permissive()
    } else {
        error!("Missing CORS configuration detected; set APP__CORS_ALLOWED_ORIGINS or APP__CORS_ALLOW_ANY_ORIGIN=true");
        return Err("Missing CORS configuration: set APP__CORS_ALLOWED_ORIGINS or APP__CORS_ALLOW_ANY_ORIGIN=true".into());
    };

    // Build router: status/health + full v1 API + Swagger UI
    let mut app = Router::<api::AppState>::new()
        .route("/", get(|| async { "stateset-api up" }))
        .route(
            "/metrics",
            get(|| async move {
                match api::metrics::metrics_handler().await {
                    Ok(body) => (StatusCode::OK, body),
                    Err(_) => (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        String::from("metrics error"),
                    ),
                }
            }),
        )
        .route(
            "/metrics/json",
            get(|| async move {
                match api::metrics::metrics_json_handler().await {
                    Ok(json) => (StatusCode::OK, axum::Json(json)),
                    Err(_) => (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        axum::Json(serde_json::json!({"error":"metrics error"})),
                    ),
                }
            }),
        )
        .nest("/api/v1", api::api_v1_routes())
        .nest(
            "/auth",
            api::auth::auth_routes().with_state(auth_service.clone()),
        )
        .merge(api::openapi::swagger_ui())
        // HTTP tracing layer for consistent request/response telemetry
        .layer(api::tracing::configure_http_tracing())
        // Apply compression and timeouts
        .layer(CompressionLayer::new())
        // Apply CORS
        .layer(cors_layer)
        // Idempotency (Redis-backed)
        .layer(axum::middleware::from_fn_with_state(
            redis_client.clone(),
            api::middleware_helpers::idempotency_redis::idempotency_redis_middleware,
        ))
        // Inject AuthService into request extensions for auth middleware
        .layer(axum::middleware::from_fn_with_state(
            auth_service.clone(),
            |axum::extract::State(auth): axum::extract::State<Arc<api::auth::AuthService>>,
             mut req: axum::http::Request<axum::body::Body>,
             next: axum::middleware::Next| async move {
                req.extensions_mut().insert(auth);
                next.run(req).await
            },
        ))
        // Ensure every request carries a request id for traceability
        .layer(axum::middleware::from_fn(
            api::middleware_helpers::request_id::request_id_middleware,
        ))
        .with_state(app_state);

    // Configure and apply global Rate Limiter layer (in-memory by default; Redis-backed planned)
    // Pull base limits from config; allow overrides via path and key policies
    let rl_cfg = api::rate_limiter::RateLimitConfig {
        requests_per_window: cfg.rate_limit_requests_per_window,
        window_duration: Duration::from_secs(cfg.rate_limit_window_seconds),
        enable_headers: cfg.rate_limit_enable_headers,
        ..Default::default()
    };

    let rl_backend = if cfg.rate_limit_use_redis {
        api::rate_limiter::RateLimitBackend::Redis {
            client: redis_client.clone(),
            namespace: cfg.rate_limit_namespace.clone(),
        }
    } else {
        api::rate_limiter::RateLimitBackend::InMemory
    };

    let mut layer = api::rate_limiter::RateLimitLayer::new(rl_cfg, rl_backend);

    // Parse rate limit policies using validated parsing (logs warnings for invalid entries)
    let parsed_policies = api::rate_limiter::parse_all_policies(
        cfg.rate_limit_path_policies.as_deref(),
        cfg.rate_limit_api_key_policies.as_deref(),
        cfg.rate_limit_user_policies.as_deref(),
    );

    // Log any warnings from policy parsing
    for warning in &parsed_policies.warnings {
        warn!("Rate limit policy configuration: {}", warning);
    }

    // Apply parsed policies
    if !parsed_policies.path_policies.is_empty() {
        info!(
            "Configured {} path-based rate limit policies",
            parsed_policies.path_policies.len()
        );
        layer = layer.with_policies(parsed_policies.path_policies);
    }

    if !parsed_policies.api_key_policies.is_empty() {
        info!(
            "Configured {} API key rate limit policies",
            parsed_policies.api_key_policies.len()
        );
        layer = layer.with_api_key_policies(parsed_policies.api_key_policies);
    }

    if !parsed_policies.user_policies.is_empty() {
        info!(
            "Configured {} user rate limit policies",
            parsed_policies.user_policies.len()
        );
        layer = layer.with_user_policies(parsed_policies.user_policies);
    }

    app = app.layer(layer);

    // Bind and serve
    let addr = SocketAddr::from(([0, 0, 0, 0], cfg.port));
    info!("🚀 stateset-api listening on http://{}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await?;

    axum::serve(listener, app.into_make_service())
        .with_graceful_shutdown(shutdown_signal())
        .await?;

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
        use tokio::signal::unix::{signal, SignalKind};

        let mut sigterm =
            signal(SignalKind::terminate()).expect("failed to install signal handler");
        sigterm.recv().await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}
