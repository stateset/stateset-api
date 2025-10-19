use axum::{extract::Query, http::StatusCode, response::Json, routing::get, Router};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::net::SocketAddr;
use tower_http::cors::{Any, CorsLayer};
use tracing::info;

#[derive(Serialize)]
struct HealthResponse {
    status: String,
    service: String,
    version: String,
    timestamp: String,
}

#[derive(Serialize)]
struct ApiInfo {
    name: String,
    version: String,
    description: String,
    endpoints: HashMap<String, String>,
}

#[derive(Deserialize)]
struct OrderQuery {
    limit: Option<u32>,
    page: Option<u32>,
    status: Option<String>,
}

#[derive(Serialize)]
struct Order {
    id: String,
    customer_id: String,
    status: String,
    total: f64,
    created_at: String,
}

async fn health() -> Result<Json<HealthResponse>, StatusCode> {
    Ok(Json(HealthResponse {
        status: "healthy".to_string(),
        service: "stateset-api".to_string(),
        version: "0.1.0".to_string(),
        timestamp: chrono::Utc::now().to_rfc3339(),
    }))
}

async fn api_info() -> Result<Json<ApiInfo>, StatusCode> {
    let mut endpoints = HashMap::new();
    endpoints.insert("/health".to_string(), "Health check endpoint".to_string());
    endpoints.insert("/api/info".to_string(), "API information".to_string());
    endpoints.insert("/api/v1/orders".to_string(), "List orders".to_string());
    endpoints.insert(
        "/api/v1/inventory".to_string(),
        "Inventory status".to_string(),
    );
    endpoints.insert(
        "/api/v1/shipments".to_string(),
        "Shipment tracking".to_string(),
    );

    Ok(Json(ApiInfo {
        name: "Stateset API".to_string(),
        version: "0.1.0".to_string(),
        description: "State-of-the-art supply chain and operations management API".to_string(),
        endpoints,
    }))
}

async fn list_orders(
    Query(params): Query<OrderQuery>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let limit = params.limit.unwrap_or(20);
    let page = params.page.unwrap_or(1);
    let status_filter = params.status.unwrap_or_else(|| "all".to_string());

    // Mock order data
    let orders = vec![
        Order {
            id: "ord_12345".to_string(),
            customer_id: "cus_67890".to_string(),
            status: "processing".to_string(),
            total: 125.50,
            created_at: "2024-06-17T10:30:00Z".to_string(),
        },
        Order {
            id: "ord_12346".to_string(),
            customer_id: "cus_67891".to_string(),
            status: "shipped".to_string(),
            total: 89.99,
            created_at: "2024-06-17T09:15:00Z".to_string(),
        },
        Order {
            id: "ord_12347".to_string(),
            customer_id: "cus_67892".to_string(),
            status: "delivered".to_string(),
            total: 245.75,
            created_at: "2024-06-16T14:22:00Z".to_string(),
        },
    ];

    let filtered_orders: Vec<&Order> = if status_filter == "all" {
        orders.iter().collect()
    } else {
        orders
            .iter()
            .filter(|order| order.status.eq_ignore_ascii_case(&status_filter))
            .collect()
    };

    let total = filtered_orders.len();
    let orders_page: Vec<&Order> = filtered_orders.into_iter().take(limit as usize).collect();

    Ok(Json(json!({
        "orders": orders_page,
        "total": total,
        "page": page,
        "limit": limit
    })))
}

async fn inventory_status() -> Result<Json<serde_json::Value>, StatusCode> {
    Ok(Json(json!({
        "inventory": [
            {
                "product_id": "prod_001",
                "sku": "WIDGET-001",
                "name": "Premium Widget",
                "quantity_on_hand": 150,
                "quantity_available": 145,
                "quantity_reserved": 5,
                "warehouse": "WH-001",
                "last_updated": "2024-06-17T12:00:00Z"
            },
            {
                "product_id": "prod_002",
                "sku": "GADGET-002",
                "name": "Smart Gadget",
                "quantity_on_hand": 75,
                "quantity_available": 70,
                "quantity_reserved": 5,
                "warehouse": "WH-001",
                "last_updated": "2024-06-17T11:30:00Z"
            }
        ],
        "total_products": 2,
        "low_stock_alerts": 0
    })))
}

async fn shipment_tracking() -> Result<Json<serde_json::Value>, StatusCode> {
    Ok(Json(json!({
        "shipments": [
            {
                "shipment_id": "ship_001",
                "order_id": "ord_12346",
                "tracking_number": "1Z999AA1234567890",
                "carrier": "UPS",
                "status": "in_transit",
                "estimated_delivery": "2024-06-18T17:00:00Z",
                "last_updated": "2024-06-17T08:45:00Z"
            },
            {
                "shipment_id": "ship_002",
                "order_id": "ord_12347",
                "tracking_number": "1Z999BB9876543210",
                "carrier": "FedEx",
                "status": "delivered",
                "delivered_at": "2024-06-16T16:30:00Z",
                "last_updated": "2024-06-16T16:30:00Z"
            }
        ],
        "total_shipments": 2,
        "in_transit": 1,
        "delivered": 1
    })))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    info!("🚀 Starting Stateset API Minimal Server...");

    // Build our application with routes
    let app = Router::new()
        .route("/health", get(health))
        .route("/api/info", get(api_info))
        .route("/api/v1/orders", get(list_orders))
        .route("/api/v1/inventory", get(inventory_status))
        .route("/api/v1/shipments", get(shipment_tracking))
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        );

    // Run it
    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));
    info!("✅ Stateset API listening on http://{}", addr);
    info!("📋 Available endpoints:");
    info!("   GET  /health            - Health check");
    info!("   GET  /api/info          - API information");
    info!("   GET  /api/v1/orders     - List orders");
    info!("   GET  /api/v1/inventory  - Inventory status");
    info!("   GET  /api/v1/shipments  - Shipment tracking");

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
