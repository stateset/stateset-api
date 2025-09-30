use axum::{
    extract::{Json, Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::signal;
use tower_http::{
    compression::CompressionLayer,
    cors::{CorsLayer, Any},
    trace::TraceLayer,
};
use tracing::info;

mod cache;
mod config;
mod delegated_payment;
mod errors;
mod events;
mod models;
mod service;

use cache::InMemoryCache;
use config::Config;
use delegated_payment::{DelegatedPaymentService, DelegatePaymentRequest};
use errors::ApiError;
use events::EventSender;
use models::*;
use service::AgenticCheckoutService;

/// Application state shared across handlers
#[derive(Clone)]
pub struct AppState {
    pub checkout_service: Arc<AgenticCheckoutService>,
    pub delegated_payment_service: Arc<DelegatedPaymentService>,
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

    // Load configuration
    let config = Config::load()?;
    info!("Configuration loaded");

    // Initialize database connection (optional - only if you need persistence)
    // For now, we'll use in-memory storage
    
    // Initialize cache
    let cache = Arc::new(InMemoryCache::new());
    info!("Cache initialized");

    // Initialize event sender
    let (event_tx, mut event_rx) = tokio::sync::mpsc::channel(1024);
    let event_sender = Arc::new(EventSender::new(event_tx));
    
    // Spawn event processor
    tokio::spawn(async move {
        while let Some(event) = event_rx.recv().await {
            info!("Event received: {:?}", event);
            // Process events (emit webhooks, etc.)
        }
    });

    // Initialize checkout service
    let checkout_service = Arc::new(AgenticCheckoutService::new(
        cache.clone(),
        event_sender,
    ));
    info!("Checkout service initialized");

    // Initialize delegated payment service
    let delegated_payment_service = Arc::new(DelegatedPaymentService::new(cache));
    info!("Delegated payment service initialized");

    // Build application state
    let app_state = AppState {
        checkout_service,
        delegated_payment_service,
    };

    // Build router
    let app = Router::new()
        // Health check
        .route("/", get(root_handler))
        .route("/health", get(health_check))
        .route("/ready", get(readiness_check))
        
        // Agentic Checkout endpoints
        .route("/checkout_sessions", post(create_checkout_session))
        .route("/checkout_sessions/:checkout_session_id", get(get_checkout_session))
        .route("/checkout_sessions/:checkout_session_id", post(update_checkout_session))
        .route("/checkout_sessions/:checkout_session_id/complete", post(complete_checkout_session))
        .route("/checkout_sessions/:checkout_session_id/cancel", post(cancel_checkout_session))
        
        // Delegated Payment endpoint (PSP mock)
        .route("/agentic_commerce/delegate_payment", post(delegate_payment))
        
        // Middleware
        .layer(CompressionLayer::new())
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any)
        )
        .layer(TraceLayer::new_for_http())
        
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

    let session = state
        .checkout_service
        .create_session(payload)
        .await?;

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

    let body = serde_json::to_string(&session)
        .map_err(|e| ApiError::InternalServerError {
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
        .await?;

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
        .await?;

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

    let body = serde_json::to_string(&session)
        .map_err(|e| ApiError::InternalServerError {
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

    let body = serde_json::to_string(&result)
        .map_err(|e| ApiError::InternalServerError {
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