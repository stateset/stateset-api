//! StateSet API Library
//!
//! This crate provides the core functionality for the StateSet API
#![forbid(unsafe_code)]
#![deny(rust_2018_idioms)]
#![allow(elided_lifetimes_in_paths)]
#![warn(clippy::all, clippy::perf, clippy::dbg_macro)]

// Core modules
pub mod api;
pub mod auth;
pub mod cache;
pub mod circuit_breaker;
pub mod commands;
pub mod config;
pub mod db;
pub mod entities;
pub mod errors;
pub mod events;
pub mod handlers;
pub mod health;
pub mod logging;
pub mod message_queue;
pub mod metrics;
pub mod middleware_helpers;
pub mod migrator;
pub mod models;
pub mod openapi;
pub mod proto;
pub mod rate_limiter;
pub mod services;
pub mod tracing;
pub mod versioning;
pub mod webhooks;

use axum::{extract::State, response::Json, routing::get, Router};
use chrono::Utc;
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;
use utoipa::ToSchema;

// Tracing imports - use external tracing crate directly to avoid conflicts

// Import handler traits
use crate::auth::consts as perm;
use crate::auth::AuthRouterExt;
use handlers::inventory::InventoryHandlerState;

// App state definition
#[derive(Clone)]
pub struct AppState {
    pub db: Arc<DatabaseConnection>,
    pub config: config::AppConfig,
    pub event_sender: events::EventSender,
    pub inventory_service: services::inventory::InventoryService,
    pub services: handlers::AppServices,
    pub redis: Arc<redis::Client>,
}

impl AppState {
    pub fn return_service(&self) -> Arc<services::returns::ReturnService> {
        self.services.returns.clone()
    }

    pub fn shipment_service(&self) -> Arc<services::shipments::ShipmentService> {
        self.services.shipments.clone()
    }

    pub fn warranty_service(&self) -> Arc<services::warranties::WarrantyService> {
        self.services.warranties.clone()
    }

    pub fn work_order_service(&self) -> Arc<services::work_orders::WorkOrderService> {
        self.services.work_orders.clone()
    }
}

// Common query parameters for list endpoints
#[derive(Debug, Deserialize, ToSchema)]
pub struct ListQuery {
    #[serde(default = "default_page")]
    pub page: u64,
    #[serde(default = "default_limit")]
    pub limit: u64,
    pub search: Option<String>,
    pub sort_by: Option<String>,
    pub sort_order: Option<String>,
}

fn default_page() -> u64 {
    1
}
fn default_limit() -> u64 {
    20
}

// Common response wrappers
#[derive(Serialize, ToSchema)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub message: Option<String>,
    pub errors: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meta: Option<ResponseMeta>,
}

#[derive(Serialize, ToSchema)]
pub struct ResponseMeta {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    pub timestamp: String,
}

impl ResponseMeta {
    fn capture() -> Self {
        Self {
            request_id: crate::tracing::current_request_id().map(|rid| rid.as_str().to_string()),
            timestamp: Utc::now().to_rfc3339(),
        }
    }
}

#[derive(Serialize, ToSchema)]
pub struct PaginatedResponse<T> {
    pub items: Vec<T>,
    pub total: u64,
    pub page: u64,
    pub limit: u64,
    pub total_pages: u64,
}

impl<T> ApiResponse<T> {
    pub fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            message: None,
            errors: None,
            meta: Some(ResponseMeta::capture()),
        }
    }

    pub fn error(message: String) -> Self {
        Self {
            success: false,
            data: None,
            message: Some(message),
            errors: None,
            meta: Some(ResponseMeta::capture()),
        }
    }

    pub fn validation_errors(errors: Vec<String>) -> Self {
        Self {
            success: false,
            data: None,
            message: Some("Validation failed".to_string()),
            errors: Some(errors),
            meta: Some(ResponseMeta::capture()),
        }
    }
}

#[cfg(test)]
mod response_tests {
    use super::*;
    use chrono::DateTime;

    #[tokio::test]
    async fn success_response_includes_request_metadata() {
        let response =
            crate::tracing::scope_request_id(crate::tracing::RequestId::new("meta-123"), async {
                ApiResponse::success("ok")
            })
            .await;

        let meta = response.meta.expect("metadata expected");
        assert_eq!(meta.request_id.as_deref(), Some("meta-123"));
        DateTime::parse_from_rfc3339(&meta.timestamp).expect("timestamp should parse");
    }

    #[tokio::test]
    async fn error_response_includes_request_metadata() {
        let response =
            crate::tracing::scope_request_id(crate::tracing::RequestId::new("meta-err"), async {
                ApiResponse::<()>::error("oops".into())
            })
            .await;

        let meta = response.meta.expect("metadata expected");
        assert_eq!(meta.request_id.as_deref(), Some("meta-err"));
        assert!(!meta.timestamp.is_empty());
    }

    #[tokio::test]
    async fn validation_errors_response_includes_metadata() {
        let response = crate::tracing::scope_request_id(
            crate::tracing::RequestId::new("meta-validation"),
            async { ApiResponse::<()>::validation_errors(vec!["missing".into()]) },
        )
        .await;

        let meta = response.meta.expect("metadata expected");
        assert_eq!(meta.request_id.as_deref(), Some("meta-validation"));
        DateTime::parse_from_rfc3339(&meta.timestamp).expect("timestamp should parse");
    }
}

/// Standard API result type for JSON responses
pub type ApiResult<T> = Result<Json<ApiResponse<T>>, errors::ServiceError>;

// Enhanced API routes function
pub fn api_v1_routes() -> Router<AppState> {
    // Orders routes with permission gating
    let orders_read = Router::new()
        .route("/orders", get(handlers::orders::list_orders))
        .route("/orders/{id}", get(handlers::orders::get_order))
        .route(
            "/orders/by-number/{order_number}",
            get(handlers::orders::get_order_by_number),
        )
        .route("/orders/{id}/items", get(handlers::orders::get_order_items))
        .with_permission(perm::ORDERS_READ);

    let orders_create = Router::new()
        .route(
            "/orders",
            axum::routing::post(handlers::orders::create_order),
        )
        .with_permission(perm::ORDERS_CREATE);

    let orders_update = Router::new()
        .route(
            "/orders/{id}",
            axum::routing::put(handlers::orders::update_order),
        )
        .route(
            "/orders/{id}/items",
            axum::routing::post(handlers::orders::add_order_item),
        )
        .route(
            "/orders/{id}/status",
            axum::routing::put(handlers::orders::update_order_status),
        )
        .route(
            "/orders/{id}/archive",
            axum::routing::post(handlers::orders::archive_order),
        )
        .with_permission(perm::ORDERS_UPDATE);

    let orders_cancel = Router::new()
        .route(
            "/orders/{id}/cancel",
            axum::routing::post(handlers::orders::cancel_order),
        )
        .with_permission(perm::ORDERS_CANCEL);

    let orders_delete = Router::new()
        .route(
            "/orders/{id}",
            axum::routing::delete(handlers::orders::delete_order),
        )
        .with_permission(perm::ORDERS_DELETE);

    // Inventory routes with permission gating
    let inventory_read = Router::new()
        .route(
            "/inventory",
            get(handlers::inventory::list_inventory::<AppState>),
        )
        .route(
            "/inventory/{id}",
            get(handlers::inventory::get_inventory::<AppState>),
        )
        .route(
            "/inventory/low-stock",
            get(handlers::inventory::get_low_stock_items::<AppState>),
        )
        .with_permission(perm::INVENTORY_READ);

    let inventory_mutate = Router::new()
        .route(
            "/inventory",
            axum::routing::post(handlers::inventory::create_inventory::<AppState>),
        )
        .route(
            "/inventory/{id}",
            axum::routing::put(handlers::inventory::update_inventory::<AppState>),
        )
        .route(
            "/inventory/{id}/reserve",
            axum::routing::post(handlers::inventory::reserve_inventory::<AppState>),
        )
        .route(
            "/inventory/{id}/release",
            axum::routing::post(handlers::inventory::release_inventory::<AppState>),
        )
        .with_permission(perm::INVENTORY_ADJUST);

    let inventory_delete = Router::new()
        .route(
            "/inventory/{id}",
            axum::routing::delete(handlers::inventory::delete_inventory::<AppState>),
        )
        .with_permission(perm::INVENTORY_ADJUST);

    // Returns routes with permission gating
    let returns_read = Router::new()
        .route("/returns", get(handlers::returns::list_returns))
        .route("/returns/{id}", get(handlers::returns::get_return))
        .with_permission(perm::RETURNS_READ);

    let returns_write = Router::new()
        .route(
            "/returns",
            axum::routing::post(handlers::returns::create_return),
        )
        .route(
            "/returns/{id}/approve",
            axum::routing::post(handlers::returns::approve_return),
        )
        .route(
            "/returns/{id}/restock",
            axum::routing::post(handlers::returns::restock_return),
        )
        .with_permission(perm::RETURNS_CREATE);

    let returns_delete = Router::new()
        // .route("/returns/{id}", axum::routing::delete(handlers::returns::delete_return::<AppState>))
        .with_permission(perm::RETURNS_REJECT);

    // Shipments routes with permission gating
    let shipments_read = Router::new()
        .route("/shipments", get(handlers::shipments::list_shipments))
        .route("/shipments/{id}", get(handlers::shipments::get_shipment))
        .route(
            "/shipments/{id}/track",
            get(handlers::shipments::track_shipment),
        )
        .route(
            "/shipments/track/{tracking_number}",
            get(handlers::shipments::track_by_number),
        )
        .with_permission(perm::SHIPMENTS_READ);

    let shipments_write = Router::new()
        .route(
            "/shipments",
            axum::routing::post(handlers::shipments::create_shipment),
        )
        .route(
            "/shipments/{id}/ship",
            axum::routing::post(handlers::shipments::mark_shipped),
        )
        .route(
            "/shipments/{id}/deliver",
            axum::routing::post(handlers::shipments::mark_delivered),
        )
        .with_permission(perm::SHIPMENTS_UPDATE);

    let shipments_delete = Router::new()
        // .route("/shipments/{id}", axum::routing::delete(handlers::shipments::delete_shipment::<AppState>))
        .with_permission(perm::SHIPMENTS_DELETE);

    // Warranties routes with permission gating
    let warranties_read = Router::new()
        .route("/warranties", get(handlers::warranties::list_warranties))
        .route("/warranties/{id}", get(handlers::warranties::get_warranty))
        .with_permission(perm::WARRANTIES_READ);

    let warranties_create = Router::new()
        .route(
            "/warranties",
            axum::routing::post(handlers::warranties::create_warranty),
        )
        .with_permission(perm::WARRANTIES_CREATE);

    let warranties_update = Router::new()
        .route(
            "/warranties/{id}/extend",
            axum::routing::post(handlers::warranties::extend_warranty),
        )
        .route(
            "/warranties/claims",
            axum::routing::post(handlers::warranties::create_warranty_claim),
        )
        .route(
            "/warranties/claims/{id}/approve",
            axum::routing::post(handlers::warranties::approve_warranty_claim),
        )
        .with_permission(perm::WARRANTIES_UPDATE);

    // Work Orders routes with permission gating
    let work_orders_read = Router::new()
        .route(
            "/work-orders",
            get(handlers::work_orders::list_work_orders::<AppState>),
        )
        .route(
            "/work-orders/{id}",
            get(handlers::work_orders::get_work_order::<AppState>),
        )
        .with_permission(perm::WORKORDERS_READ);

    let work_orders_create = Router::new()
        .route(
            "/work-orders",
            axum::routing::post(handlers::work_orders::create_work_order::<AppState>),
        )
        .with_permission(perm::WORKORDERS_CREATE);

    let work_orders_update = Router::new()
        .route(
            "/work-orders/{id}",
            axum::routing::put(handlers::work_orders::update_work_order::<AppState>),
        )
        .route(
            "/work-orders/{id}/assign",
            axum::routing::post(handlers::work_orders::assign_work_order::<AppState>),
        )
        .route(
            "/work-orders/{id}/complete",
            axum::routing::post(handlers::work_orders::complete_work_order::<AppState>),
        )
        .route(
            "/work-orders/{id}/status",
            axum::routing::put(handlers::work_orders::update_work_order_status::<AppState>),
        )
        .with_permission(perm::WORKORDERS_UPDATE);

    let work_orders_delete = Router::new()
        .route(
            "/work-orders/{id}",
            axum::routing::delete(handlers::work_orders::delete_work_order::<AppState>),
        )
        .with_permission(perm::WORKORDERS_DELETE);

    let manufacturing_boms = handlers::bom::bom_routes().with_permission(perm::BOMS_MANAGE);

    // Admin outbox routes
    let outbox_admin = handlers::outbox_admin::router().with_permission("admin:outbox");

    // Payments routes
    let payments = handlers::payments::payment_routes().with_permission("payments:access");
    // Payment webhook (does not require auth, but signature-verified)
    let payment_webhook = Router::new().route(
        "/payments/webhook",
        axum::routing::post(handlers::payment_webhooks::payment_webhook),
    );

    // Procurement routes
    let purchase_orders = handlers::purchase_orders::purchase_order_routes()
        .with_permission(perm::PURCHASEORDERS_MANAGE);
    let asns = handlers::asn::asn_routes().with_permission(perm::ASNS_MANAGE);
    let analytics = handlers::analytics::analytics_routes().with_permission(perm::ANALYTICS_READ);

    Router::new()
        // Status and health endpoints
        .route("/status", get(api_status))
        .route("/health", get(health_check))
        // Orders API (auth + permissions)
        .merge(orders_read)
        .merge(orders_create)
        .merge(orders_update)
        .merge(orders_cancel)
        .merge(orders_delete)
        // Inventory API (auth + permissions)
        .merge(inventory_read)
        .merge(inventory_mutate)
        .merge(inventory_delete)
        // ASN API (auth + permissions) - temporarily disabled
        // .route("/asns", get(handlers::asn::list_asns))
        // .route("/asns/{id}", get(handlers::asn::get_asn))
        // .route("/asns", post(handlers::asn::create_asn))
        // .route("/asns/{id}", put(handlers::asn::update_asn))
        // .route("/asns/{id}", delete(handlers::asn::delete_asn))
        // .route("/asns/{id}/in-transit", post(handlers::asn::in_transit_asn))
        // .route("/asns/{id}/delivered", post(handlers::asn::delivered_asn))
        // .route("/asns/{id}/cancel", post(handlers::asn::cancel_asn))
        // .with_permission("asn:read")
        // Returns API (auth + permissions)
        .merge(returns_read)
        .merge(returns_write)
        .merge(returns_delete)
        // Shipments API (auth + permissions)
        .merge(shipments_read)
        .merge(shipments_write)
        .merge(shipments_delete)
        // Warranties API (auth + permissions)
        .merge(warranties_read)
        .merge(warranties_create)
        .merge(warranties_update)
        // Agents API
        .nest("/agents", handlers::agents::agents_routes())
        // Work Orders API (auth + permissions)
        .merge(work_orders_read)
        .merge(work_orders_create)
        .merge(work_orders_update)
        .merge(work_orders_delete)
        // Manufacturing BOM API
        .nest("/manufacturing/boms", manufacturing_boms)
        // Payments API
        .nest("/payments", payments)
        .merge(payment_webhook)
        // Procurement
        .nest("/purchase-orders", purchase_orders)
        .nest("/asns", asns)
        // Commerce API (products, carts, checkout)
        .nest("/products", handlers::commerce::products_routes())
        .nest("/carts", handlers::commerce::carts_routes())
        .nest("/checkout", handlers::commerce::checkout_routes())
        .nest("/customers", handlers::commerce::customers_routes())
        // Agentic Checkout API (for ChatGPT integration)
        .merge(handlers::commerce::agentic_checkout_routes())
        // Admin
        .nest("/admin/outbox", outbox_admin)
        // Analytics
        .nest("/analytics", analytics)
}

async fn api_status() -> Result<Json<ApiResponse<Value>>, errors::ServiceError> {
    let version = env!("CARGO_PKG_VERSION");
    let git = option_env!("GIT_HASH").unwrap_or("unknown");
    let build_time = option_env!("BUILD_TIME").unwrap_or("unknown");
    let status_data = json!({
        "status": "ok",
        "version": version,
        "git": git,
        "build_time": build_time,
        "service": "stateset-api",
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "environment": std::env::var("ENVIRONMENT").unwrap_or_else(|_| "development".to_string()),
    });

    Ok(Json(ApiResponse::success(status_data)))
}

async fn health_check(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Value>>, errors::ServiceError> {
    // Check database connectivity
    let db_status = match state.db.ping().await {
        Ok(_) => "healthy",
        Err(_) => "unhealthy",
    };

    // Check Redis connectivity
    let redis_status = match state.redis.get_async_connection().await {
        Ok(mut conn) => match redis::cmd("PING").query_async::<_, String>(&mut conn).await {
            Ok(_) => "healthy",
            Err(_) => "unhealthy",
        },
        Err(_) => "unhealthy",
    };

    let health_data = json!({
        "status": if db_status == "healthy" && redis_status == "healthy" { "healthy" } else { "unhealthy" },
        "checks": {
            "database": db_status,
            "cache": redis_status,
            "message_queue": "unknown",
        },
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "uptime": "unknown", // TODO: Calculate actual uptime
    });

    Ok(Json(ApiResponse::success(health_data)))
}

// Request logging middleware
async fn request_logging_middleware(
    request: axum::http::Request<axum::body::Body>,
    next: axum::middleware::Next,
) -> axum::response::Response {
    let method = request.method().clone();
    let uri = request.uri().clone();
    let start = std::time::Instant::now();

    // Log incoming request
    tracing::info!(method = %method, uri = %uri, "Incoming request");

    let response = next.run(request).await;

    let duration = start.elapsed();
    let status = response.status();

    // Log completed request
    tracing::info!(
        method = %method,
        uri = %uri,
        status = status.as_u16(),
        elapsed_ms = duration.as_millis() as u64,
        "Request completed"
    );

    response
}

pub mod prelude {
    pub use crate::api::*;
    // pub use crate::cache::*;
    // pub use crate::commands::*;
    pub use crate::db::*;
    pub use crate::errors::*;
    pub use crate::events::*;
    pub use crate::health::*;
    pub use crate::metrics::*;
    // pub use crate::models::*;
    pub use crate::openapi::*;
    // Note: proto and services both export modules with the same names (inventory, billofmaterials)
    // Import them under namespaced prefixes to avoid ambiguous glob re-exports
    pub use crate::proto as grpc_proto;
    // pub use crate::queries::*;
    pub use crate::rate_limiter::*;
    pub use crate::services::*;
    pub use crate::tracing::*;
    pub use crate::versioning::*;
}

// Note: AppState automatically implements ReturnsAppState, ShipmentsAppState,
// WarrantiesAppState, and WorkOrdersAppState through blanket implementations
// in the respective handler modules
