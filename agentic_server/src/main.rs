use axum::{
    extract::{Json, Path, Request, State},
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    routing::{get, post},
    Router,
};
use std::sync::Arc;
use std::{net::SocketAddr, time::Instant};
use tokio::signal;
use tower_http::{
    compression::CompressionLayer,
    cors::{Any, CorsLayer},
    limit::RequestBodyLimitLayer,
    trace::TraceLayer,
};
use tracing::{info, warn};

mod auth;
mod cache;
mod config;
mod constants;
mod delegated_payment;
mod errors;
mod events;
mod idempotency;
mod metrics;
mod models;
mod product_catalog;
mod rate_limit;
mod redis_store;
mod security;
mod service;
mod shopify_integration;
mod stripe_integration;
mod tax_service;
mod validation;
mod webhook_service;

use auth::{auth_middleware, ApiKeyStore};
use cache::InMemoryCache;
use config::Config;
use constants::MAX_REQUEST_BODY_BYTES;
use delegated_payment::{DelegatePaymentRequest, DelegatedPaymentService};
use errors::ApiError;
use events::{Event, EventSender};
use idempotency::{idempotency_middleware, IdempotencyService};
use metrics::{
    record_http_request, CHECKOUT_SESSIONS_CREATED, ORDERS_CREATED, VAULT_TOKENS_CREATED,
};
use models::*;
use product_catalog::ProductCatalogService;
use rate_limit::{rate_limit_middleware, RateLimiter};
use redis_store::RedisStore;
use security::{signature_verification_middleware, SignatureVerifier};
use service::AgenticCheckoutService;
use shopify_integration::{ShopifyClient, ShopifyConfig};
use stripe_integration::{StripeConfig, StripePaymentProcessor};
use tax_service::TaxService;
use webhook_service::WebhookService;

/// Application state shared across handlers
#[derive(Clone)]
pub struct AppState {
    pub checkout_service: Arc<AgenticCheckoutService>,
    pub delegated_payment_service: Arc<DelegatedPaymentService>,
    pub rate_limiter: RateLimiter,
    pub api_key_store: ApiKeyStore,
    pub signature_verifier: Option<Arc<SignatureVerifier>>,
    pub idempotency_service: Option<Arc<IdempotencyService>>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_target(false)
        .with_thread_ids(true)
        .with_level(true)
        .json()
        .init();

    info!("Starting Agentic Commerce Server...");

    // Load environment variables from .env if present
    let _ = dotenvy::dotenv();

    // Load configuration
    let config = Config::load()?;
    info!(
        "Configuration loaded (host: {}, port: {}, webhook_url_configured: {})",
        config.host,
        config.port,
        config.webhook_url.as_ref().map(|_| "yes").unwrap_or("no")
    );

    // Initialize signature verifier (optional - set secret to enable)
    let signature_verifier = std::env::var("WEBHOOK_SECRET").ok().map(|secret| {
        info!("Signature verification enabled");
        Arc::new(SignatureVerifier::new(secret))
    });

    // Initialize database connection (optional - only if you need persistence)
    // For now, we'll use in-memory storage

    // Initialize cache
    let cache = Arc::new(InMemoryCache::new());
    info!("Cache initialized");

    // Initialize event sender
    let (event_tx, mut event_rx) = tokio::sync::mpsc::channel(1024);
    let event_sender = Arc::new(EventSender::new(event_tx));

    // Initialize webhook service
    let webhook_service = Arc::new(WebhookService::new(signature_verifier.clone()));
    info!("Webhook service initialized");

    // Spawn event processor
    let webhook_service_events = webhook_service.clone();
    let webhook_url = config.webhook_url.clone();
    if webhook_url.is_none() {
        info!("WEBHOOK_URL not set; outbound webhook delivery disabled");
    }
    tokio::spawn(async move {
        while let Some(event) = event_rx.recv().await {
            info!("Event received: {:?}", event);

            if let (
                Some(url),
                Event::CheckoutCompleted {
                    session_id,
                    order_id,
                },
            ) = (&webhook_url, &event)
            {
                let session_id_str = session_id.to_string();
                let order_id_str = order_id.to_string();
                let permalink = format!("https://merchant.example.com/orders/{}", order_id);

                if let Err(err) = webhook_service_events
                    .send_order_created(
                        url,
                        session_id_str.clone(),
                        order_id_str.clone(),
                        permalink.clone(),
                    )
                    .await
                {
                    warn!("Webhook delivery failed: {}", err);
                }

                if let Err(err) = webhook_service_events
                    .send_order_updated(
                        url,
                        session_id_str,
                        permalink,
                        "created".to_string(),
                        vec![],
                    )
                    .await
                {
                    warn!("Webhook update delivery failed: {}", err);
                }
            }
        }
    });

    // Initialize product catalog
    let product_catalog = Arc::new(ProductCatalogService::new());
    info!("Product catalog initialized (3 products)");

    // Initialize tax service
    let tax_service = Arc::new(TaxService::new());
    info!("Tax service initialized (5 jurisdictions)");

    // Initialize Stripe integration (optional)
    let stripe_processor = match StripeConfig::from_env() {
        Ok(config) => {
            info!("Stripe integration enabled");
            Some(Arc::new(StripePaymentProcessor::new(config)))
        }
        Err(err) => {
            info!("Stripe integration disabled: {}", err);
            None
        }
    };

    // Initialize Shopify integration (optional)
    let shopify_client = match ShopifyConfig::from_env() {
        Ok(Some(config)) => match ShopifyClient::new(config) {
            Ok(client) => {
                info!("Shopify integration enabled");
                Some(Arc::new(client))
            }
            Err(err) => {
                warn!("Failed to initialize Shopify client: {}", err);
                None
            }
        },
        Ok(None) => {
            info!("Shopify integration disabled");
            None
        }
        Err(err) => {
            warn!(
                "Shopify integration disabled due to configuration error: {}",
                err
            );
            None
        }
    };

    // Initialize checkout service
    let checkout_service = Arc::new(AgenticCheckoutService::new(
        cache.clone(),
        event_sender,
        product_catalog,
        tax_service,
        stripe_processor.clone(),
        shopify_client.clone(),
    ));
    info!("Checkout service initialized");

    // Initialize delegated payment service
    let delegated_payment_service = Arc::new(DelegatedPaymentService::new(cache));
    info!("Delegated payment service initialized");

    // Initialize rate limiter (100 requests per minute)
    let rate_limiter = RateLimiter::new(100);
    info!("Rate limiter initialized (100 req/min)");

    // Initialize API key store
    let api_key_store = ApiKeyStore::new();
    info!("API key store initialized ({} keys)", 2);

    // Initialize signature verifier (optional - set secret to enable)
    let signature_verifier = std::env::var("WEBHOOK_SECRET").ok().map(|secret| {
        info!("Signature verification enabled");
        Arc::new(SignatureVerifier::new(secret))
    });

    // Initialize Redis store (optional - falls back to in-memory)
    let redis_store = if let Ok(redis_url) = std::env::var("REDIS_URL") {
        match RedisStore::new(&redis_url).await {
            Ok(store) => {
                info!("Redis store connected: {}", redis_url);
                Some(Arc::new(store))
            }
            Err(e) => {
                warn!("Redis connection failed, using in-memory cache: {}", e);
                None
            }
        }
    } else {
        info!("REDIS_URL not set, using in-memory cache");
        None
    };

    // Initialize idempotency service (requires Redis)
    let idempotency_service = redis_store.clone().map(|redis| {
        info!("Idempotency service initialized (Redis-backed)");
        Arc::new(IdempotencyService::new(redis))
    });

    // Build application state
    let app_state = AppState {
        checkout_service,
        delegated_payment_service,
        rate_limiter: rate_limiter.clone(),
        api_key_store: api_key_store.clone(),
        signature_verifier: signature_verifier.clone(),
        idempotency_service: idempotency_service.clone(),
    };

    // Build router
    let app = Router::new()
        // Health check
        .route("/", get(root_handler))
        .route("/health", get(health_check))
        .route("/ready", get(readiness_check))
        .route("/metrics", get(metrics_handler))
        // Agentic Checkout endpoints
        .route("/checkout_sessions", post(create_checkout_session))
        .route(
            "/checkout_sessions/:checkout_session_id",
            get(get_checkout_session),
        )
        .route(
            "/checkout_sessions/:checkout_session_id",
            post(update_checkout_session),
        )
        .route(
            "/checkout_sessions/:checkout_session_id/complete",
            post(complete_checkout_session),
        )
        .route(
            "/checkout_sessions/:checkout_session_id/cancel",
            post(cancel_checkout_session),
        )
        // Delegated Payment endpoint (PSP mock)
        .route("/agentic_commerce/delegate_payment", post(delegate_payment))
        // Middleware layers (applied in reverse order: bottom to top)
        // Order: metrics -> rate_limit -> idempotency -> auth -> signature -> trace -> CORS -> compression -> body_limit
        .layer(RequestBodyLimitLayer::new(MAX_REQUEST_BODY_BYTES))
        .layer(CompressionLayer::new())
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        )
        .layer(TraceLayer::new_for_http())
        .layer(axum::middleware::from_fn_with_state(
            signature_verifier.clone(),
            signature_verification_middleware,
        ))
        .layer(axum::middleware::from_fn_with_state(
            api_key_store.clone(),
            auth_middleware,
        ))
        .layer(axum::middleware::from_fn_with_state(
            idempotency_service.clone(),
            idempotency_middleware,
        ))
        // Rate limiting before auth to prevent brute force attempts
        .layer(axum::middleware::from_fn_with_state(
            rate_limiter.clone(),
            rate_limit_middleware,
        ))
        .layer(axum::middleware::from_fn(metrics_middleware))
        .with_state(app_state);

    // Bind to address
    let addr: SocketAddr = format!("{}:{}", config.host, config.port)
        .parse()
        .expect("Invalid address");

    info!("Server listening on {}", addr);

    // Start server
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

// Handler functions

async fn root_handler() -> &'static str {
    "Agentic Commerce Server - Ready for ChatGPT Checkout"
}

async fn health_check() -> impl IntoResponse {
    Json(serde_json::json!({
        "status": "healthy",
        "service": "agentic-commerce",
        "version": env!("CARGO_PKG_VERSION")
    }))
}

async fn readiness_check() -> impl IntoResponse {
    Json(serde_json::json!({
        "ready": true
    }))
}

async fn metrics_handler() -> impl IntoResponse {
    match metrics::gather_metrics() {
        Ok(metrics_text) => (StatusCode::OK, metrics_text).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to gather metrics: {}", e),
        )
            .into_response(),
    }
}

async fn metrics_middleware(request: Request, next: Next) -> Response {
    let method = request.method().clone();
    let path = request.uri().path().to_owned();
    let start = Instant::now();

    let response = next.run(request).await;
    let status = response.status().as_u16();

    record_http_request(method.as_str(), &path, status, start);

    response
}

/// Create a checkout session
async fn create_checkout_session(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CheckoutSessionCreateRequest>,
) -> Result<Response, ApiError> {
    // Validate required items
    if payload.items.is_empty() {
        return Err(ApiError::BadRequest {
            message: "At least one item is required".to_string(),
            error_code: Some("INVALID_REQUEST".to_string()),
        });
    }

    let session = state.checkout_service.create_session(payload).await?;

    // Record metrics
    CHECKOUT_SESSIONS_CREATED.inc();

    // Build response with headers
    let mut response = Response::builder()
        .status(StatusCode::CREATED)
        .header("Content-Type", "application/json");

    // Echo idempotency key if provided
    if let Some(idempotency_key) = headers.get("Idempotency-Key") {
        response = response.header("Idempotency-Key", idempotency_key);
    }

    // Echo request ID if provided
    if let Some(request_id) = headers.get("Request-Id") {
        response = response.header("Request-Id", request_id);
    }

    let body = serde_json::to_string(&session).map_err(|e| ApiError::InternalServerError {
        message: format!("Serialization error: {}", e),
    })?;

    Ok(response
        .body(body.into())
        .map_err(|e| ApiError::InternalServerError {
            message: format!("Response build error: {}", e),
        })?)
}

/// Get checkout session
async fn get_checkout_session(
    State(state): State<AppState>,
    Path(checkout_session_id): Path<String>,
) -> Result<Json<CheckoutSession>, ApiError> {
    let session = state
        .checkout_service
        .get_session(&checkout_session_id)
        .await?;

    Ok(Json(session))
}

/// Update checkout session
async fn update_checkout_session(
    State(state): State<AppState>,
    Path(checkout_session_id): Path<String>,
    Json(payload): Json<CheckoutSessionUpdateRequest>,
) -> Result<Json<CheckoutSession>, ApiError> {
    let session = state
        .checkout_service
        .update_session(&checkout_session_id, payload)
        .await
        .map_err(|e| {
            tracing::error!("Update session failed: {:?}", e);
            ApiError::ServiceError(e)
        })?;

    Ok(Json(session))
}

/// Complete checkout session
async fn complete_checkout_session(
    State(state): State<AppState>,
    Path(checkout_session_id): Path<String>,
    Json(payload): Json<CheckoutSessionCompleteRequest>,
) -> Result<Json<CheckoutSessionWithOrder>, ApiError> {
    let result = state
        .checkout_service
        .complete_session(&checkout_session_id, payload)
        .await
        .map_err(|e| {
            tracing::error!("Complete session failed: {:?}", e);
            ApiError::ServiceError(e)
        })?;

    // Record metrics
    metrics::CHECKOUT_COMPLETIONS.inc();
    ORDERS_CREATED.inc();

    Ok(Json(result))
}

/// Cancel checkout session
async fn cancel_checkout_session(
    State(state): State<AppState>,
    Path(checkout_session_id): Path<String>,
) -> Result<Response, ApiError> {
    let session = state
        .checkout_service
        .cancel_session(&checkout_session_id)
        .await?;

    let body = serde_json::to_string(&session).map_err(|e| ApiError::InternalServerError {
        message: format!("Serialization error: {}", e),
    })?;

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "application/json")
        .body(body.into())
        .map_err(|e| ApiError::InternalServerError {
            message: format!("Response build error: {}", e),
        })?)
}

/// Delegate payment (PSP endpoint)
async fn delegate_payment(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<DelegatePaymentRequest>,
) -> Result<Response, ApiError> {
    let result = state
        .delegated_payment_service
        .delegate_payment(payload)
        .await
        .map_err(|e| match e {
            crate::errors::ServiceError::InvalidInput(msg) => ApiError::BadRequest {
                message: msg,
                error_code: Some("invalid_card".to_string()),
            },
            crate::errors::ServiceError::InvalidOperation(msg) => ApiError::BadRequest {
                message: msg,
                error_code: Some("processing_error".to_string()),
            },
            _ => ApiError::InternalServerError {
                message: "Failed to process delegated payment".to_string(),
            },
        })?;

    // Record metrics
    VAULT_TOKENS_CREATED.inc();

    // Build response with headers
    let mut response = Response::builder()
        .status(StatusCode::CREATED)
        .header("Content-Type", "application/json");

    // Echo idempotency key if provided
    if let Some(idempotency_key) = headers.get("Idempotency-Key") {
        response = response.header("Idempotency-Key", idempotency_key);
    }

    // Echo request ID if provided
    if let Some(request_id) = headers.get("Request-Id") {
        response = response.header("Request-Id", request_id);
    }

    let body = serde_json::to_string(&result).map_err(|e| ApiError::InternalServerError {
        message: format!("Serialization error: {}", e),
    })?;

    Ok(response
        .body(body.into())
        .map_err(|e| ApiError::InternalServerError {
            message: format!("Response build error: {}", e),
        })?)
}

/// Graceful shutdown signal handler
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
        _ = ctrl_c => {
            info!("Received Ctrl+C signal");
        },
        _ = terminate => {
            info!("Received terminate signal");
        },
    }

    info!("Shutting down gracefully...");
}
