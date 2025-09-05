use std::{net::SocketAddr, sync::Arc, time::Duration};

use axum::{
    routing::get,
    Router,
};
use tokio::signal;
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

    // Init events
    let (event_tx, event_rx) = tokio::mpsc::channel(1024);
    let event_sender = api::events::EventSender::new(event_tx);
    tokio::spawn(api::events::process_events(event_rx));

    // Build services
    let db_arc = Arc::new(db_pool);
    let inventory_service = api::services::inventory::InventoryService::new(
        db_arc.clone(),
        event_sender.clone(),
    );

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

    // Build router: status/health + full v1 API + Swagger UI
    let app = Router::new()
        .route("/", get(|| async { "stateset-api up" }))
        .merge(api::openapi::swagger_ui())
        .nest("/api/v1", api::api_v1_routes())
        // Inject AuthService into request extensions for auth middleware
        .layer(axum::middleware::from_fn_with_state(
            auth_service.clone(),
            |axum::extract::State(auth): axum::extract::State<Arc<api::auth::AuthService>>, mut req, next| async move {
                req.extensions_mut().insert(auth);
                next.run(req).await
            },
        ))
        .with_state(app_state);

    // Bind and serve
    let addr = SocketAddr::from(([0, 0, 0, 0], cfg.port));
    info!("🚀 stateset-api listening on http://{}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await?;

    axum::serve(listener, app)
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

        let mut sigterm = signal(SignalKind::terminate())
            .expect("failed to install signal handler");
        sigterm.recv().await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}
