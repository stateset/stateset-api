/*!
 * # OpenAPI Documentation Module
 *
 * This module provides comprehensive API documentation using the OpenAPI 3.0
 * specification. It includes:
 *
 * - Swagger UI for interactive API exploration
 * - Separate documentation for each API version
 * - Security scheme documentation
 * - Complete request/response schema documentation
 * - Endpoint, parameter, and response descriptions
 */

use crate::{
    auth,
    errors::{ErrorResponse},
    // Remove handler imports since handlers module doesn't exist
    versioning::ApiVersion,
};
use axum::{extract::Path, http::StatusCode, response::IntoResponse, routing::get, Router};
use axum::Json;
use std::sync::Arc;
use utoipa::{
    openapi::security::{ApiKey, ApiKeyValue, Http, HttpAuthScheme, HttpBuilder, SecurityScheme},
    IntoParams, Modify, OpenApi,
};
use utoipa_swagger_ui::{Config, SwaggerUi};
use uuid::Uuid;

/// API documentation module
///
/// This module configures and provides a Swagger UI for the API
/// using OpenAPI 3.0 specification.

/// OpenAPI documentation for API v1
#[derive(OpenApi)]
#[openapi(
    info(
        title = "Stateset API",
        version = "1.0.0",
        description = "Stateset API for order, inventory, and supply chain management",
        license(
            name = "MIT",
            url = "https://opensource.org/licenses/MIT"
        ),
        contact(
            name = "Stateset API Team",
            url = "https://stateset.io",
            email = "support@stateset.io"
        ),
    ),
    servers(
        (url = "/api/v1", description = "Production API v1"),
        (url = "/api/v1-beta", description = "Beta API v1")
    ),
    paths(
        // TODO: Uncomment when handlers module is implemented
        // create_order,
        // get_order,
        // update_order_status,
        // list_orders,
        // cancel_order,
        // add_order_item,
    ),
    components(
        schemas(
            // Only include types that actually exist
            super::errors::ErrorResponse,
            // TODO: Add order-related schemas when handlers module is implemented
        )
    ),
    tags(
        (name = "orders", description = "Order management endpoints"),
        (name = "inventory", description = "Inventory management endpoints"),
        (name = "returns", description = "Returns processing endpoints"),
        (name = "warranties", description = "Warranty management endpoints"),
        (name = "shipments", description = "Shipment tracking endpoints"),
        (name = "work-orders", description = "Work order management endpoints"),
    ),
    modifiers(&SecurityAddon)
)]
pub struct ApiDocV1;

/// OpenAPI documentation for API v2 (future)
#[derive(OpenApi)]
#[openapi(
    info(
        title = "Stateset API",
        version = "2.0.0-alpha",
        description = "Stateset API v2 (Alpha) for order, inventory, and supply chain management",
        license(
            name = "MIT",
            url = "https://opensource.org/licenses/MIT"
        ),
        contact(
            name = "Stateset API Team",
            url = "https://stateset.io",
            email = "support@stateset.io"
        ),
    ),
    servers(
        (url = "/api/v2", description = "Alpha API v2")
    ),
    // V2 paths will be added here
    components(
        schemas(
            // V2 schemas will be added here
            super::errors::ErrorResponse,
            // TODO: Re-enable once ServiceError implements ToSchema properly
            // OrderError,
            // InventoryError,
        )
    ),
    tags(
        (name = "orders", description = "Order management endpoints"),
        (name = "inventory", description = "Inventory management endpoints"),
        (name = "returns", description = "Returns processing endpoints"),
        (name = "warranties", description = "Warranty management endpoints"),
        (name = "shipments", description = "Shipment tracking endpoints"),
        (name = "work-orders", description = "Work order management endpoints"),
    ),
    modifiers(&SecurityAddon)
)]
pub struct ApiDocV2;

/// Security scheme modifier for OpenAPI docs
pub struct SecurityAddon;

impl Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        // Add API key auth
        if let Some(components) = &mut openapi.components {
            components.add_security_scheme(
                "api_key",
                SecurityScheme::ApiKey(ApiKey::Header(ApiKeyValue::new("X-API-Key"))),
            );
            // Add JWT bearer auth
            components.add_security_scheme(
                "jwt_auth",
                SecurityScheme::Http(HttpBuilder::new().scheme(HttpAuthScheme::Bearer).bearer_format("JWT").build()),
            );
            // Add OAuth2 (if implemented in the future)
        }
    }
}

/// Lightweight Swagger UI + OpenAPI JSON routes for Axum
pub fn swagger_routes() -> Router {
    Router::new().merge(
        SwaggerUi::new("/docs")
            .url("/api-docs/v1/openapi.json", ApiDocV1::openapi())
            .url("/api-docs/v2/openapi.json", ApiDocV2::openapi()),
    )
}

/// Get OpenAPI specs for a specific version
pub async fn openapi_json(Path(version): Path<String>) -> impl IntoResponse {
    match version.as_str() {
        "v1" => (StatusCode::OK, Json(serde_json::to_value(ApiDocV1::openapi()).unwrap())),
        "v2" => (StatusCode::OK, Json(serde_json::to_value(ApiDocV2::openapi()).unwrap())),
        _ => (StatusCode::NOT_FOUND, Json(serde_json::to_value("API version not found").unwrap())),
    }
}

/// Handler for API version documentation
pub async fn openapi_handler(Path(version): Path<String>) -> impl IntoResponse {
    let openapi_path = match version.as_str() {
        "v1" => Some("/api-docs/v1/openapi.json"),
        "v2" => Some("/api-docs/v2/openapi.json"),
        _ => None,
    };

    if let Some(_path) = openapi_path {
        // UI is mounted via swagger_routes() at /docs
        (StatusCode::OK, "Swagger UI available at /docs").into_response()
    } else {
        (StatusCode::NOT_FOUND, "API version not found").into_response()
    }
}

/// Documentation home page listing available API versions
pub async fn docs_home() -> impl IntoResponse {
    let html = r#"<!DOCTYPE html>
<html>
<head>
    <title>Stateset API Documentation</title>
    <style>
        body {
            font-family: system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif;
            line-height: 1.5;
            max-width: 1000px;
            margin: 0 auto;
            padding: 2rem;
            color: #333;
        }
        h1 { color: #2563eb; margin-bottom: 2rem; }
        h2 { margin-top: 2rem; color: #1d4ed8; }
        a {
            color: #2563eb;
            text-decoration: none;
        }
        a:hover { text-decoration: underline; }
        .version-card {
            border: 1px solid #e5e7eb;
            border-radius: 0.5rem;
            padding: 1.5rem;
            margin-bottom: 1.5rem;
            box-shadow: 0 1px 3px rgba(0,0,0,0.1);
        }
        .version-header {
            display: flex;
            justify-content: space-between;
            align-items: center;
            margin-bottom: 1rem;
        }
        .version-name {
            font-size: 1.5rem;
            font-weight: bold;
            margin: 0;
        }
        .version-status {
            display: inline-block;
            padding: 0.25rem 0.75rem;
            border-radius: 9999px;
            font-size: 0.875rem;
            font-weight: 500;
            text-transform: uppercase;
        }
        .status-stable { background-color: #dcfce7; color: #166534; }
        .status-beta { background-color: #ffedd5; color: #9a3412; }
        .status-alpha { background-color: #fee2e2; color: #b91c1c; }
        .status-deprecated { background-color: #f3f4f6; color: #6b7280; }
        .version-buttons {
            display: flex;
            gap: 1rem;
            margin-top: 1rem;
        }
        .btn {
            display: inline-flex;
            align-items: center;
            padding: 0.5rem 1rem;
            border-radius: 0.375rem;
            font-weight: 500;
            transition: all 0.15s ease;
        }
        .btn-primary {
            background-color: #2563eb;
            color: white;
        }
        .btn-secondary {
            background-color: #f3f4f6;
            color: #4b5563;
        }
        .btn:hover {
            opacity: 0.9;
            text-decoration: none;
        }
    </style>
</head>
<body>
    <h1>Stateset API Documentation</h1>
    
    <p>
        Welcome to the Stateset API documentation. This page provides access to documentation
        for all available API versions.
    </p>
    
    <h2>Available API Versions</h2>
    
    <div class="version-card">
        <div class="version-header">
            <h3 class="version-name">API v1</h3>
            <span class="version-status status-stable">Stable</span>
        </div>
        <div class="version-description">
            <p>The current stable API version for production use. This version is fully supported and recommended for all integrations.</p>
            <p><strong>Released:</strong> January 2023</p>
        </div>
        <div class="version-buttons">
            <a href="/docs" class="btn btn-primary">Open Swagger UI</a>
            <a href="/api-docs/v1/openapi.json" class="btn btn-secondary">OpenAPI Spec</a>
        </div>
    </div>
    
    <div class="version-card">
        <div class="version-header">
            <h3 class="version-name">API v2</h3>
            <span class="version-status status-alpha">Alpha</span>
        </div>
        <div class="version-description">
            <p>The next generation API currently in development. This version is not recommended for production use yet.</p>
            <p><strong>Released:</strong> June 2024 (Alpha)</p>
        </div>
        <div class="version-buttons">
            <a href="/docs" class="btn btn-primary">Open Swagger UI</a>
            <a href="/api-docs/v2/openapi.json" class="btn btn-secondary">OpenAPI Spec</a>
        </div>
    </div>
    
    <h2>API Versioning</h2>
    <p>
        The Stateset API uses semantic versioning to manage changes. You can specify the API version in your requests using one of these methods:
    </p>
    <ul>
        <li>URL path: <code>/api/v1/orders</code></li>
        <li>Accept header: <code>Accept: application/vnd.stateset.v1+json</code></li>
        <li>Version header: <code>X-API-Version: 1</code></li>
    </ul>
    
    <h2>Additional Resources</h2>
    <ul>
        <li><a href="/api/versions">View API versions information</a></li>
        <li><a href="https://stateset.io/docs">Documentation Portal</a></li>
        <li><a href="https://stateset.io/changelog">API Changelog</a></li>
    </ul>
</body>
</html>"#;

    (StatusCode::OK, [("Content-Type", "text/html")], html).into_response()
}

/// Create a router with Swagger UI documentation for all API versions
pub fn create_docs_routes() -> Router {
    Router::new()
        .route("/", get(docs_home))
        .route("/:version", get(openapi_handler))
        .route("/openapi/:version", get(openapi_json))
}

/*
// TODO: Implement these API endpoints when handlers module exists

#[utoipa::path(
    post,
    path = "/api/v1/orders",
    request_body = CreateOrderRequest,
    responses(
        (status = 201, description = "Order created successfully", body = OrderResponse),
        (status = 400, description = "Bad request", body = super::errors::ErrorResponse),
        (status = 401, description = "Unauthorized", body = super::errors::ErrorResponse),
        (status = 422, description = "Unprocessable entity", body = super::errors::ErrorResponse),
        (status = 500, description = "Internal server error", body = super::errors::ErrorResponse),
    ),
    security(
        ("jwt_auth" = [])
    )
)]
pub async fn create_order() {}

/// Get order by ID
#[utoipa::path(
    get,
    path = "/api/v1/orders/{id}",
    params(
        ("id" = Uuid, Path, description = "Order ID")
    ),
    responses(
        (status = 200, description = "Order found", body = OrderResponse),
        (status = 401, description = "Unauthorized", body = super::errors::ErrorResponse),
        (status = 403, description = "Forbidden", body = super::errors::ErrorResponse),
        (status = 404, description = "Order not found", body = super::errors::ErrorResponse),
        (status = 500, description = "Internal server error", body = super::errors::ErrorResponse),
    ),
    security(
        ("jwt_auth" = [])
    ),
    tag = "orders"
)]
async fn get_order() {}

/// Update order status
#[utoipa::path(
    put,
    path = "/api/v1/orders/{id}/status",
    params(
        ("id" = Uuid, Path, description = "Order ID")
    ),
    request_body = UpdateOrderStatusRequest,
    responses(
        (status = 200, description = "Order status updated", body = OrderResponse),
        (status = 400, description = "Bad request", body = super::errors::ErrorResponse),
        (status = 401, description = "Unauthorized", body = super::errors::ErrorResponse),
        (status = 403, description = "Forbidden", body = super::errors::ErrorResponse),
        (status = 404, description = "Order not found", body = super::errors::ErrorResponse),
        (status = 422, description = "Invalid status transition", body = super::errors::ErrorResponse),
        (status = 500, description = "Internal server error", body = super::errors::ErrorResponse),
    ),
    security(
        ("jwt_auth" = [])
    ),
    tag = "orders"
)]
async fn update_order_status() {}

/// List orders
#[utoipa::path(
    get,
    path = "/api/v1/orders",
    params(
        OrderSearchParams
    ),
    responses(
        (status = 200, description = "List of orders", body = OrderListResponse),
        (status = 400, description = "Bad request", body = super::errors::ErrorResponse),
        (status = 401, description = "Unauthorized", body = super::errors::ErrorResponse),
        (status = 500, description = "Internal server error", body = super::errors::ErrorResponse),
    ),
    security(
        ("jwt_auth" = [])
    ),
    tag = "orders"
)]
async fn list_orders() {}

/// Cancel order
#[utoipa::path(
    post,
    path = "/api/v1/orders/{id}/cancel",
    params(
        ("id" = Uuid, Path, description = "Order ID")
    ),
    request_body = CancelOrderRequest,
    responses(
        (status = 200, description = "Order cancelled", body = OrderResponse),
        (status = 400, description = "Bad request", body = super::errors::ErrorResponse),
        (status = 401, description = "Unauthorized", body = super::errors::ErrorResponse),
        (status = 403, description = "Forbidden", body = super::errors::ErrorResponse),
        (status = 404, description = "Order not found", body = super::errors::ErrorResponse),
        (status = 422, description = "Order cannot be cancelled", body = super::errors::ErrorResponse),
    ),
    security(
        ("jwt_auth" = [])
    ),
    tag = "orders"
)]
async fn cancel_order() {}

/// Add item to order
#[utoipa::path(
    post,
    path = "/api/v1/orders/{id}/items",
    params(
        ("id" = Uuid, Path, description = "Order ID")
    ),
    request_body = AddOrderItemRequest,
    responses(
        (status = 200, description = "Item added", body = OrderItemResponse),
        (status = 400, description = "Bad request", body = super::errors::ErrorResponse),
        (status = 401, description = "Unauthorized", body = super::errors::ErrorResponse),
        (status = 403, description = "Forbidden", body = super::errors::ErrorResponse),
        (status = 404, description = "Order not found", body = super::errors::ErrorResponse),
        (status = 422, description = "Invalid item or insufficient inventory", body = super::errors::ErrorResponse),
    ),
    security(
        ("jwt_auth" = [])
    ),
    tag = "orders"
)]
async fn add_item_to_order() {}
*/

// Basic placeholder types for minimal OpenAPI compilation
#[derive(utoipa::ToSchema, serde::Serialize)]
struct BasicResponse {
    message: String,
}
