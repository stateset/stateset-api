use std::{net::SocketAddr, sync::Arc, time::Duration};

use axum::http::StatusCode;
use axum::{routing::get, Router};
use http::HeaderValue;
use tokio::{signal, sync::mpsc};
use tower_http::{
    compression::CompressionLayer,
    cors::{Any, CorsLayer},
};
use tracing::{error, info};

use stateset_api as api;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cfg = api::config::load_config()?;
    api::config::init_tracing(cfg.log_level(), cfg.log_json);

    // Init DB
    let db_pool = api::db::establish_connection_from_app_config(&cfg).await?;
    if cfg.auto_migrate {
        if let Err(e) = api::db::run_migrations(&db_pool).await {
            error!("Failed running migrations: {}", e);
        }
    }

    // Init Redis client (construction only; connection checked in health)
    let redis_client = Arc::new(redis::Client::open(cfg.redis_url.clone())?);

    // Build services
    let db_arc = Arc::new(db_pool);
    // Init events
    let (event_tx, event_rx) = mpsc::channel(1024);
    let event_sender = api::events::EventSender::new(event_tx);
    tokio::spawn(api::events::process_events(event_rx));
    // Start outbox worker (best-effort, no-op if table missing)
    api::events::outbox::start_worker(db_arc.clone(), event_sender.clone()).await;
    let inventory_service =
        api::services::inventory::InventoryService::new(db_arc.clone(), event_sender.clone());

    // Auth service for handlers/services requiring it
    let auth_cfg = api::auth::AuthConfig::new(
        cfg.jwt_secret.clone(),
        "stateset-api".to_string(),
        "stateset-auth".to_string(),
        Duration::from_secs(cfg.jwt_expiration as u64),
        Duration::from_secs(cfg.refresh_token_expiration as u64),
        "sk_".to_string(),
    );
    let auth_service = Arc::new(api::auth::AuthService::new(auth_cfg, db_arc.clone()));

    // Aggregate app services used by HTTP handlers
    let services = api::handlers::AppServices::new(
        db_arc.clone(),
        Arc::new(event_sender.clone()),
        redis_client.clone(),
        auth_service.clone(),
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
    let cors_layer = if cfg
        .cors_allowed_origins
        .as_ref()
        .map(|s| !s.is_empty())
        .unwrap_or(false)
    {
        let origins: Vec<HeaderValue> = cfg
            .cors_allowed_origins
            .as_ref()
            .unwrap()
            .split(',')
            .filter_map(|o| HeaderValue::from_str(o.trim()).ok())
            .collect();
        CorsLayer::new()
            .allow_origin(origins)
            .allow_methods(Any)
            .allow_headers(Any)
            .allow_credentials(cfg.cors_allow_credentials)
    } else {
        // Permissive for local/dev
        CorsLayer::permissive()
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

    let mut layer = api::rate_limiter::RateLimitLayer::new(rl_cfg);

    // Optional: path policy overrides in config (format: "/api/v1/orders:60:60,/api/v1/inventory:120:60")
    if let Some(policies) = &cfg.rate_limit_path_policies {
        let mut parsed = Vec::new();
        for spec in policies
            .split(',')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
        {
            let parts: Vec<&str> = spec.split(':').collect();
            if parts.len() == 3 {
                if let (Ok(limit), Ok(win)) = (parts[1].parse::<u32>(), parts[2].parse::<u64>()) {
                    parsed.push(api::rate_limiter::PathPolicy {
                        prefix: parts[0].to_string(),
                        requests_per_window: limit,
                        window_duration: Duration::from_secs(win),
                    });
                }
            }
        }
        if !parsed.is_empty() {
            layer = layer.with_policies(parsed);
        }
    }

    // Per API key policies: "key1:200:60,key2:1000:60"
    if let Some(api_key_specs) = &cfg.rate_limit_api_key_policies {
        let mut map = std::collections::HashMap::new();
        for spec in api_key_specs
            .split(',')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
        {
            let parts: Vec<&str> = spec.split(':').collect();
            if parts.len() == 3 {
                if let (Ok(limit), Ok(win)) = (parts[1].parse::<u32>(), parts[2].parse::<u64>()) {
                    map.insert(parts[0].to_string(), (limit, Duration::from_secs(win)));
                }
            }
        }
        if !map.is_empty() {
            layer = layer.with_api_key_policies(map);
        }
    }

    // Per user policies: "user123:500:60,user456:50:60"
    if let Some(user_specs) = &cfg.rate_limit_user_policies {
        let mut map = std::collections::HashMap::new();
        for spec in user_specs
            .split(',')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
        {
            let parts: Vec<&str> = spec.split(':').collect();
            if parts.len() == 3 {
                if let (Ok(limit), Ok(win)) = (parts[1].parse::<u32>(), parts[2].parse::<u64>()) {
                    map.insert(parts[0].to_string(), (limit, Duration::from_secs(win)));
                }
            }
        }
        if !map.is_empty() {
            layer = layer.with_user_policies(map);
        }
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
