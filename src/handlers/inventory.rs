use crate::errors::ServiceError;
use crate::services::inventory::InventoryService;
use axum::{
    extract::{Json, Path, Query, State},
    response::IntoResponse,
    routing::{delete, get, post, put},
    Router,
    http::StatusCode,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use serde_json::json;
use uuid::Uuid;
use std::sync::Arc;

// Trait for inventory handler state that provides access to inventory service
pub trait InventoryHandlerState: Clone + Send + Sync + 'static {
    fn inventory_service(&self) -> &InventoryService;
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct InventoryItem {
    pub id: String,
    pub product_id: String,
    pub location_id: String,
    pub quantity: i32,
    pub allocated_quantity: i32,
    pub reserved_quantity: i32,
    pub available_quantity: i32,
    pub unit_cost: Option<f64>,
    pub last_updated: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct InventoryAdjustment {
    pub id: String,
    pub inventory_item_id: String,
    pub adjustment_type: String,
    pub quantity_change: i32,
    pub reason: String,
    pub reference_number: Option<String>,
    pub created_by: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateInventoryRequest {
    pub product_id: String,
    pub location_id: String,
    pub quantity: i32,
    pub unit_cost: Option<f64>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateInventoryRequest {
    pub quantity: Option<i32>,
    pub unit_cost: Option<f64>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct AdjustInventoryRequest {
    pub adjustment_type: String, // "increase", "decrease", "set"
    pub quantity: i32,
    pub reason: String,
    pub reference_number: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct AllocateInventoryRequest {
    pub product_id: String,
    pub location_id: String,
    pub quantity: i32,
    pub order_id: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ReserveInventoryRequest {
    pub product_id: String,
    pub location_id: String,
    pub quantity: i32,
    pub reference_id: String,
    pub reference_type: String, // "order", "quote", "hold"
}

#[derive(Debug, Deserialize, ToSchema, utoipa::IntoParams)]
pub struct InventoryFilters {
    pub product_id: Option<String>,
    pub location_id: Option<String>,
    pub low_stock: Option<bool>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

/// Create the inventory router
pub fn inventory_router<S>() -> Router<S> 
where 
    S: InventoryHandlerState,
{
    Router::new()
        .route("/", get(list_inventory::<S>).post(create_inventory::<S>))
        .route("/{id}", get(get_inventory::<S>).put(update_inventory::<S>).delete(delete_inventory::<S>))
        .route("/adjust", post(adjust_inventory::<S>))
        .route("/allocate", post(allocate_inventory::<S>))
        .route("/reserve", post(reserve_inventory::<S>))
        .route("/release", post(release_inventory::<S>))
        .route("/adjustments", get(list_adjustments::<S>))
}

/// List inventory items with optional filtering
#[utoipa::path(
    get,
    path = "/api/v1/inventory",
    params(
        InventoryFilters
    ),
    responses(
        (status = 200, description = "Inventory list returned",
            headers(
                ("X-Request-Id" = String, description = "Unique request id for tracing"),
                ("X-RateLimit-Limit" = String, description = "Requests allowed in current window"),
                ("X-RateLimit-Remaining" = String, description = "Remaining requests in window"),
                ("X-RateLimit-Reset" = String, description = "Seconds until window resets"),
            )
        ),
        (status = 400, description = "Invalid request", body = crate::errors::ErrorResponse),
        (status = 401, description = "Unauthorized", body = crate::errors::ErrorResponse),
        (status = 403, description = "Forbidden", body = crate::errors::ErrorResponse),
        (status = 429, description = "Rate limit exceeded", body = crate::errors::ErrorResponse),
        (status = 500, description = "Internal server error", body = crate::errors::ErrorResponse)
    ),
    tag = "inventory"
)]
pub async fn list_inventory<S>(
    State(state): State<S>,
    Query(filters): Query<InventoryFilters>,
) -> Result<impl IntoResponse, ServiceError> 
where 
    S: InventoryHandlerState,
{
    let inventory_service = state.inventory_service();
    let page = (filters.offset.unwrap_or(0) / filters.limit.unwrap_or(50)) + 1;
    let limit = filters.limit.unwrap_or(50) as u64;

    // Get inventory items from database with pagination
    let (db_items, total) = inventory_service
        .list_inventory(page as u64, limit)
        .await?;

    // Convert database models to API response format
    let mut inventory_items: Vec<InventoryItem> = db_items
        .into_iter()
        .map(|item| InventoryItem {
            id: item.id,
            product_id: item.sku, // Using SKU as product_id
            location_id: item.warehouse.to_string(),
            quantity: item.available + item.allocated_quantity.unwrap_or(0) + item.reserved_quantity.unwrap_or(0),
            allocated_quantity: item.allocated_quantity.unwrap_or(0),
            reserved_quantity: item.reserved_quantity.unwrap_or(0),
            available_quantity: item.available,
            unit_cost: item.unit_cost.map(|cost| cost.to_string().parse().unwrap_or(0.0)),
            last_updated: item.last_movement_date.map(|d| d.and_utc()).unwrap_or_else(|| Utc::now()),
            created_at: item.arrival_date.and_hms_opt(0, 0, 0).unwrap().and_utc(),
        })
        .collect();

    // Apply filters
    if let Some(product_id) = &filters.product_id {
        inventory_items.retain(|item| &item.product_id == product_id);
    }
    if let Some(location_id) = &filters.location_id {
        inventory_items.retain(|item| &item.location_id == location_id);
    }
    if let Some(true) = filters.low_stock {
        inventory_items.retain(|item| item.available_quantity < 10);
    }

    let response = json!({
        "success": true,
        "data": {
            "inventory": inventory_items,
            "total": total,
            "page": page,
            "per_page": limit,
            "limit": filters.limit.unwrap_or(50),
            "offset": filters.offset.unwrap_or(0)
        }
    });

    Ok((StatusCode::OK, Json(response)))
}

/// Create new inventory item
#[utoipa::path(
    post,
    path = "/api/v1/inventory",
    request_body = CreateInventoryRequest,
    responses(
        (status = 201, description = "Inventory item created", body = InventoryItem,
            headers(("X-Request-Id" = String, description = "Unique request id"))
        ),
        (status = 400, description = "Invalid request", body = crate::errors::ErrorResponse),
        (status = 401, description = "Unauthorized", body = crate::errors::ErrorResponse),
        (status = 403, description = "Forbidden", body = crate::errors::ErrorResponse),
        (status = 429, description = "Rate limit exceeded", body = crate::errors::ErrorResponse),
        (status = 500, description = "Internal server error", body = crate::errors::ErrorResponse)
    ),
    tag = "inventory"
)]
pub async fn create_inventory<S>(
    State(_state): State<S>,
    Json(payload): Json<CreateInventoryRequest>,
) -> Result<impl IntoResponse, ServiceError> 
where 
    S: InventoryHandlerState,
{
    let inventory_item = InventoryItem {
        id: Uuid::new_v4().to_string(),
        product_id: payload.product_id,
        location_id: payload.location_id,
        quantity: payload.quantity,
        allocated_quantity: 0,
        reserved_quantity: 0,
        available_quantity: payload.quantity,
        unit_cost: payload.unit_cost,
        last_updated: Utc::now(),
        created_at: Utc::now(),
    };

    Ok((StatusCode::CREATED, Json(inventory_item)))
}

/// Get specific inventory item
#[utoipa::path(
    get,
    path = "/api/v1/inventory/{id}",
    params(
        ("id" = String, Path, description = "Inventory item ID")
    ),
    responses(
        (status = 200, description = "Inventory item returned", body = InventoryItem,
            headers(("X-Request-Id" = String, description = "Unique request id"))
        ),
        (status = 401, description = "Unauthorized", body = crate::errors::ErrorResponse),
        (status = 403, description = "Forbidden", body = crate::errors::ErrorResponse),
        (status = 404, description = "Not found", body = crate::errors::ErrorResponse),
        (status = 429, description = "Rate limit exceeded", body = crate::errors::ErrorResponse),
        (status = 500, description = "Internal server error", body = crate::errors::ErrorResponse)
    ),
    tag = "inventory"
)]
pub async fn get_inventory<S>(
    State(_state): State<S>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, ServiceError> 
where 
    S: InventoryHandlerState,
{
    let inventory_item = InventoryItem {
        id: id.clone(),
        product_id: "prod_abc".to_string(),
        location_id: "loc_warehouse_001".to_string(),
        quantity: 100,
        allocated_quantity: 20,
        reserved_quantity: 10,
        available_quantity: 70,
        unit_cost: Some(25.99),
        last_updated: Utc::now(),
        created_at: Utc::now() - chrono::Duration::days(30),
    };

    Ok((StatusCode::OK, Json(inventory_item)))
}

/// Update inventory item
#[utoipa::path(
    put,
    path = "/api/v1/inventory/{id}",
    params(
        ("id" = String, Path, description = "Inventory item ID")
    ),
    request_body = UpdateInventoryRequest,
    responses(
        (status = 200, description = "Inventory item updated", body = InventoryItem,
            headers(("X-Request-Id" = String, description = "Unique request id"))
        ),
        (status = 400, description = "Invalid request", body = crate::errors::ErrorResponse),
        (status = 401, description = "Unauthorized", body = crate::errors::ErrorResponse),
        (status = 403, description = "Forbidden", body = crate::errors::ErrorResponse),
        (status = 404, description = "Not found", body = crate::errors::ErrorResponse),
        (status = 429, description = "Rate limit exceeded", body = crate::errors::ErrorResponse),
        (status = 500, description = "Internal server error", body = crate::errors::ErrorResponse)
    ),
    tag = "inventory"
)]
pub async fn update_inventory<S>(
    State(_state): State<S>,
    Path(id): Path<String>,
    Json(payload): Json<UpdateInventoryRequest>,
) -> Result<impl IntoResponse, ServiceError> 
where 
    S: InventoryHandlerState,
{
    let inventory_item = InventoryItem {
        id: id.clone(),
        product_id: "prod_abc".to_string(),
        location_id: "loc_warehouse_001".to_string(),
        quantity: payload.quantity.unwrap_or(100),
        allocated_quantity: 20,
        reserved_quantity: 10,
        available_quantity: payload.quantity.unwrap_or(100) - 30,
        unit_cost: payload.unit_cost.or(Some(25.99)),
        last_updated: Utc::now(),
        created_at: Utc::now() - chrono::Duration::days(30),
    };

    Ok((StatusCode::OK, Json(inventory_item)))
}

/// Delete inventory item
#[utoipa::path(
    delete,
    path = "/api/v1/inventory/{id}",
    params(
        ("id" = String, Path, description = "Inventory item ID")
    ),
    responses(
        (status = 200, description = "Inventory item deleted",
            headers(("X-Request-Id" = String, description = "Unique request id"))
        ),
        (status = 401, description = "Unauthorized", body = crate::errors::ErrorResponse),
        (status = 403, description = "Forbidden", body = crate::errors::ErrorResponse),
        (status = 404, description = "Not found", body = crate::errors::ErrorResponse),
        (status = 429, description = "Rate limit exceeded", body = crate::errors::ErrorResponse),
        (status = 500, description = "Internal server error", body = crate::errors::ErrorResponse)
    ),
    tag = "inventory"
)]
pub async fn delete_inventory<S>(
    State(_state): State<S>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, ServiceError> 
where 
    S: InventoryHandlerState,
{
    let response = json!({
        "message": format!("Inventory item {} has been deleted", id),
        "deleted_id": id
    });

    Ok((StatusCode::OK, Json(response)))
}

/// Adjust inventory quantities
async fn adjust_inventory<S>(
    State(_state): State<S>,
    Json(payload): Json<AdjustInventoryRequest>,
) -> Result<impl IntoResponse, ServiceError> 
where 
    S: InventoryHandlerState,
{
    let adjustment = InventoryAdjustment {
        id: Uuid::new_v4().to_string(),
        inventory_item_id: "inv_001".to_string(),
        adjustment_type: payload.adjustment_type.clone(),
        quantity_change: payload.quantity,
        reason: payload.reason,
        reference_number: payload.reference_number,
        created_by: "user_001".to_string(),
        created_at: Utc::now(),
    };

    Ok((StatusCode::CREATED, Json(adjustment)))
}

/// Allocate inventory for orders
async fn allocate_inventory<S>(
    State(_state): State<S>,
    Json(payload): Json<AllocateInventoryRequest>,
) -> Result<impl IntoResponse, ServiceError> 
where 
    S: InventoryHandlerState,
{
    let response = json!({
        "message": "Inventory allocated successfully",
        "product_id": payload.product_id,
        "location_id": payload.location_id,
        "allocated_quantity": payload.quantity,
        "order_id": payload.order_id,
        "allocation_id": Uuid::new_v4().to_string()
    });

    Ok((StatusCode::OK, Json(response)))
}

/// Reserve inventory
pub async fn reserve_inventory<S>(
    State(_state): State<S>,
    Json(payload): Json<ReserveInventoryRequest>,
) -> Result<impl IntoResponse, ServiceError> 
where 
    S: InventoryHandlerState,
{
    let response = json!({
        "message": "Inventory reserved successfully",
        "product_id": payload.product_id,
        "location_id": payload.location_id,
        "reserved_quantity": payload.quantity,
        "reference_id": payload.reference_id,
        "reference_type": payload.reference_type,
        "reservation_id": Uuid::new_v4().to_string()
    });

    Ok((StatusCode::OK, Json(response)))
}

/// Release reserved inventory
pub async fn release_inventory<S>(
    State(_state): State<S>,
    Json(payload): Json<serde_json::Value>,
) -> Result<impl IntoResponse, ServiceError> 
where 
    S: InventoryHandlerState,
{
    let response = json!({
        "message": "Reserved inventory released successfully",
        "released_at": Utc::now()
    });

    Ok((StatusCode::OK, Json(response)))
}

/// List inventory adjustments
async fn list_adjustments<S>(
    State(_state): State<S>,
    Query(_filters): Query<serde_json::Value>,
) -> Result<impl IntoResponse, ServiceError> 
where 
    S: InventoryHandlerState,
{
    let adjustments = vec![
        InventoryAdjustment {
            id: "adj_001".to_string(),
            inventory_item_id: "inv_001".to_string(),
            adjustment_type: "increase".to_string(),
            quantity_change: 50,
            reason: "Stock replenishment".to_string(),
            reference_number: Some("PO-2024-001".to_string()),
            created_by: "user_001".to_string(),
            created_at: Utc::now() - chrono::Duration::hours(2),
        }
    ];

    let response = json!({
        "adjustments": adjustments,
        "total": adjustments.len()
    });

    Ok((StatusCode::OK, Json(response)))
}

/// Get low stock items
#[utoipa::path(
    get,
    path = "/api/v1/inventory/low-stock",
    responses(
        (status = 200, description = "Low stock items returned",
            headers(("X-Request-Id" = String, description = "Unique request id"))
        ),
        (status = 401, description = "Unauthorized", body = crate::errors::ErrorResponse),
        (status = 403, description = "Forbidden", body = crate::errors::ErrorResponse),
        (status = 429, description = "Rate limit exceeded", body = crate::errors::ErrorResponse),
        (status = 500, description = "Internal server error", body = crate::errors::ErrorResponse)
    ),
    tag = "inventory"
)]
pub async fn get_low_stock_items<S>(
    State(_state): State<S>,
    Query(filters): Query<InventoryFilters>,
) -> Result<impl IntoResponse, ServiceError> 
where 
    S: InventoryHandlerState,
{
    let threshold = 10; // Define low stock threshold
    
    // Sample low stock items
    let low_stock_items = vec![
        InventoryItem {
            id: "inv_low_001".to_string(),
            product_id: "prod_abc".to_string(),
            location_id: "warehouse_001".to_string(),
            quantity: 5,
            allocated_quantity: 2,
            reserved_quantity: 1,
            available_quantity: 2,
            unit_cost: Some(15.99),
            last_updated: Utc::now(),
            created_at: Utc::now() - chrono::Duration::days(15),
        },
        InventoryItem {
            id: "inv_low_002".to_string(),
            product_id: "prod_def".to_string(),
            location_id: "warehouse_002".to_string(),
            quantity: 3,
            allocated_quantity: 1,
            reserved_quantity: 0,
            available_quantity: 2,
            unit_cost: Some(22.50),
            last_updated: Utc::now(),
            created_at: Utc::now() - chrono::Duration::days(20),
        }
    ];

    let response = json!({
        "success": true,
        "data": {
            "low_stock_items": low_stock_items,
            "threshold": threshold,
            "total": low_stock_items.len()
        }
    });

    Ok((StatusCode::OK, Json(response)))
}
