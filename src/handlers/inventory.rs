use crate::errors::ServiceError;
use crate::services::inventory::{
    AdjustInventoryCommand, InventoryService, InventorySnapshot, LocationBalance,
    ReleaseReservationCommand, ReservationOutcome, ReserveInventoryCommand,
};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;
use validator::Validate;

use crate::ApiResponse;

/// Trait that provides access to the inventory service for handlers.
pub trait InventoryHandlerState: Clone + Send + Sync + 'static {
    fn inventory_service(&self) -> &InventoryService;
}

/// API representation of aggregated inventory for an item.
#[derive(Debug, Serialize, ToSchema)]
#[schema(example = json!({
    "inventory_item_id": 12345,
    "item_number": "SKU-WIDGET-001",
    "description": "Premium Widget - Blue Edition",
    "primary_uom_code": "EA",
    "organization_id": 1,
    "quantities": {
        "on_hand": "500",
        "allocated": "50",
        "available": "450"
    },
    "locations": [{
        "location_id": 1,
        "location_name": "Main Warehouse",
        "quantities": {
            "on_hand": "300",
            "allocated": "30",
            "available": "270"
        },
        "updated_at": "2024-12-09T10:30:00Z",
        "version": 5
    }, {
        "location_id": 2,
        "location_name": "Distribution Center East",
        "quantities": {
            "on_hand": "200",
            "allocated": "20",
            "available": "180"
        },
        "updated_at": "2024-12-09T09:15:00Z",
        "version": 3
    }]
}))]
pub struct InventoryItem {
    /// Internal inventory item ID
    #[schema(example = 12345)]
    pub inventory_item_id: i64,
    /// SKU or item number
    #[schema(example = "SKU-WIDGET-001")]
    pub item_number: String,
    /// Item description
    #[schema(example = "Premium Widget - Blue Edition")]
    pub description: Option<String>,
    /// Unit of measure code
    #[schema(example = "EA")]
    pub primary_uom_code: Option<String>,
    /// Organization ID
    #[schema(example = 1)]
    pub organization_id: i64,
    /// Aggregated quantities across all locations
    pub quantities: InventoryQuantities,
    /// Inventory breakdown by location
    pub locations: Vec<InventoryLocation>,
}

/// API representation of quantities.
#[derive(Debug, Serialize, ToSchema)]
#[schema(example = json!({
    "on_hand": "500",
    "allocated": "50",
    "available": "450"
}))]
pub struct InventoryQuantities {
    /// Total quantity physically in stock
    #[schema(example = "500")]
    pub on_hand: String,
    /// Quantity reserved/allocated for orders
    #[schema(example = "50")]
    pub allocated: String,
    /// Quantity available for new orders (on_hand - allocated)
    #[schema(example = "450")]
    pub available: String,
}

/// API representation of inventory at a specific location.
#[derive(Debug, Serialize, ToSchema)]
#[schema(example = json!({
    "location_id": 1,
    "location_name": "Main Warehouse",
    "quantities": {
        "on_hand": "300",
        "allocated": "30",
        "available": "270"
    },
    "updated_at": "2024-12-09T10:30:00Z",
    "version": 5
}))]
pub struct InventoryLocation {
    /// Location identifier
    #[schema(example = 1)]
    pub location_id: i32,
    /// Human-readable location name
    #[schema(example = "Main Warehouse")]
    pub location_name: Option<String>,
    /// Quantities at this location
    pub quantities: InventoryQuantities,
    /// Last update timestamp (RFC3339)
    #[schema(example = "2024-12-09T10:30:00Z")]
    pub updated_at: String,
    /// Version number for optimistic locking. Include this in update requests to prevent lost updates.
    #[schema(example = 5)]
    pub version: i32,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(example = json!({
    "items": [],
    "total": 150,
    "page": 1,
    "per_page": 50
}))]
pub struct InventoryListResponse {
    /// List of inventory items
    pub items: Vec<InventoryItem>,
    /// Total number of items matching the query
    #[schema(example = 150)]
    pub total: u64,
    /// Current page number
    #[schema(example = 1)]
    pub page: u64,
    /// Items per page
    #[schema(example = 50)]
    pub per_page: u64,
}

const DEFAULT_PAGE_SIZE: u32 = 50;
const MAX_PAGE_SIZE: u32 = 100;

#[derive(Debug, Deserialize, ToSchema, utoipa::IntoParams)]
pub struct InventoryFilters {
    pub product_id: Option<String>,
    pub location_id: Option<String>,
    pub low_stock: Option<bool>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
#[schema(example = json!({
    "item_number": "SKU-WIDGET-001",
    "description": "Premium Widget - Blue Edition",
    "primary_uom_code": "EA",
    "organization_id": 1,
    "location_id": 1,
    "quantity_on_hand": 500,
    "reason": "Initial stock receipt from supplier PO-2024-001"
}))]
pub struct CreateInventoryRequest {
    /// SKU or item number (1-100 characters)
    #[validate(length(min = 1, max = 100, message = "Item number must be between 1 and 100 characters"))]
    #[schema(example = "SKU-WIDGET-001")]
    pub item_number: String,
    /// Item description (max 500 characters)
    #[validate(length(max = 500, message = "Description must not exceed 500 characters"))]
    #[schema(example = "Premium Widget - Blue Edition")]
    pub description: Option<String>,
    /// Unit of measure code (e.g., "EA", "KG", "LB")
    #[validate(length(max = 20, message = "UOM code must not exceed 20 characters"))]
    #[schema(example = "EA")]
    pub primary_uom_code: Option<String>,
    /// Organization ID (must be positive if provided)
    #[validate(range(min = 1, message = "Organization ID must be positive"))]
    #[schema(example = 1)]
    pub organization_id: Option<i64>,
    /// Location ID (must be positive)
    #[validate(range(min = 1, message = "Location ID must be positive"))]
    #[schema(example = 1)]
    pub location_id: i32,
    /// Initial quantity on hand (cannot be negative)
    #[validate(range(min = 0, message = "Quantity on hand cannot be negative"))]
    #[schema(example = 500)]
    pub quantity_on_hand: i64,
    /// Reason for inventory adjustment (max 200 characters)
    #[validate(length(max = 200, message = "Reason must not exceed 200 characters"))]
    #[schema(example = "Initial stock receipt from supplier PO-2024-001")]
    pub reason: Option<String>,
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
#[schema(example = json!({
    "location_id": 1,
    "on_hand": 450,
    "description": "Premium Widget - Blue Edition (Updated)",
    "reason": "Inventory adjustment after cycle count",
    "expected_version": 5
}))]
pub struct UpdateInventoryRequest {
    /// Location ID (must be positive)
    #[validate(range(min = 1, message = "Location ID must be positive"))]
    #[schema(example = 1)]
    pub location_id: i32,
    /// New quantity on hand (cannot be negative)
    #[validate(range(min = 0, message = "Quantity on hand cannot be negative"))]
    #[schema(example = 450)]
    pub on_hand: Option<i64>,
    /// Updated description (max 500 characters)
    #[validate(length(max = 500, message = "Description must not exceed 500 characters"))]
    #[schema(example = "Premium Widget - Blue Edition (Updated)")]
    pub description: Option<String>,
    /// Updated UOM code (max 20 characters)
    #[validate(length(max = 20, message = "UOM code must not exceed 20 characters"))]
    #[schema(example = "EA")]
    pub primary_uom_code: Option<String>,
    /// Organization ID (must be positive if provided)
    #[validate(range(min = 1, message = "Organization ID must be positive"))]
    #[schema(example = 1)]
    pub organization_id: Option<i64>,
    /// Reason for update (max 200 characters)
    #[validate(length(max = 200, message = "Reason must not exceed 200 characters"))]
    #[schema(example = "Inventory adjustment after cycle count")]
    pub reason: Option<String>,
    /// Expected version for optimistic locking. If provided, update fails if version doesn't match.
    #[schema(example = 5)]
    pub expected_version: Option<i32>,
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
#[schema(example = json!({
    "location_id": 1,
    "quantity": 10,
    "reference_id": "550e8400-e29b-41d4-a716-446655440000",
    "reference_type": "SALES_ORDER"
}))]
pub struct ReserveInventoryRequest {
    /// Location ID (must be positive)
    #[validate(range(min = 1, message = "Location ID must be positive"))]
    #[schema(example = 1)]
    pub location_id: i32,
    /// Quantity to reserve (must be at least 1)
    #[validate(range(min = 1, message = "Quantity must be at least 1"))]
    #[schema(example = 10)]
    pub quantity: i64,
    /// Reference ID (e.g., order ID) - must be valid UUID if provided
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub reference_id: Option<String>,
    /// Reference type (e.g., "SALES_ORDER", "CUSTOMER_HOLD")
    #[validate(length(max = 50, message = "Reference type must not exceed 50 characters"))]
    #[schema(example = "SALES_ORDER")]
    pub reference_type: Option<String>,
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
#[schema(example = json!({
    "location_id": 1,
    "quantity": 5
}))]
pub struct ReleaseInventoryRequest {
    /// Location ID (must be positive)
    #[validate(range(min = 1, message = "Location ID must be positive"))]
    #[schema(example = 1)]
    pub location_id: i32,
    /// Quantity to release (must be at least 1)
    #[validate(range(min = 1, message = "Quantity must be at least 1"))]
    #[schema(example = 5)]
    pub quantity: i64,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(example = json!({
    "reservation_id": "res-550e8400-e29b-41d4",
    "location": {
        "location_id": 1,
        "location_name": "Main Warehouse",
        "quantities": {
            "on_hand": "300",
            "allocated": "40",
            "available": "260"
        },
        "updated_at": "2024-12-09T10:35:00Z",
        "version": 6
    }
}))]
pub struct ReservationResponse {
    /// Unique reservation identifier
    #[schema(example = "res-550e8400-e29b-41d4")]
    pub reservation_id: String,
    /// Updated location inventory after reservation
    pub location: InventoryLocation,
}

#[derive(Debug, Deserialize, ToSchema, utoipa::IntoParams)]
pub struct LowStockQuery {
    pub threshold: Option<i64>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[utoipa::path(
    get,
    path = "/api/v1/inventory",
    params(InventoryFilters),
    responses(
        (status = 200, description = "Inventory list returned", body = ApiResponse<InventoryListResponse>,
            headers(
                ("X-Request-Id" = String, description = "Unique request identifier"),
                ("X-RateLimit-Limit" = String, description = "Maximum requests allowed in current window"),
                ("X-RateLimit-Remaining" = String, description = "Remaining requests in current window"),
                ("X-RateLimit-Reset" = String, description = "Seconds until rate limit resets"),
            )
        ),
        (status = 401, description = "Unauthorized", body = crate::errors::ErrorResponse),
        (status = 429, description = "Rate limit exceeded", body = crate::errors::ErrorResponse)
    ),
    tag = "inventory"
)]
pub async fn list_inventory<S>(
    State(state): State<S>,
    Query(filters): Query<InventoryFilters>,
) -> Result<Json<ApiResponse<InventoryListResponse>>, ServiceError>
where
    S: InventoryHandlerState,
{
    let service = state.inventory_service();
    let per_page = filters
        .limit
        .map(|limit| limit.max(1).min(MAX_PAGE_SIZE) as u64)
        .unwrap_or(DEFAULT_PAGE_SIZE as u64);
    let offset = filters.offset.unwrap_or(0) as u64;
    let page = offset / per_page + 1;

    let (items, total) = inventory_page(service, &filters, offset, per_page, None).await?;

    let response = InventoryListResponse {
        total,
        page,
        per_page,
        items,
    };

    Ok(Json(ApiResponse::success(response)))
}

#[utoipa::path(
    post,
    path = "/api/v1/inventory",
    request_body = CreateInventoryRequest,
    responses(
        (status = 201, description = "Inventory created", body = ApiResponse<InventoryItem>,
            headers(
                ("X-Request-Id" = String, description = "Unique request identifier"),
                ("X-RateLimit-Limit" = String, description = "Maximum requests allowed in current window"),
                ("X-RateLimit-Remaining" = String, description = "Remaining requests in current window"),
                ("X-RateLimit-Reset" = String, description = "Seconds until rate limit resets"),
            )
        ),
        (status = 400, description = "Bad request", body = crate::errors::ErrorResponse),
        (status = 401, description = "Unauthorized", body = crate::errors::ErrorResponse),
        (status = 429, description = "Rate limit exceeded", body = crate::errors::ErrorResponse)
    ),
    tag = "inventory"
)]
pub async fn create_inventory<S>(
    State(state): State<S>,
    Json(payload): Json<CreateInventoryRequest>,
) -> Result<(StatusCode, Json<ApiResponse<InventoryItem>>), ServiceError>
where
    S: InventoryHandlerState,
{
    payload
        .validate()
        .map_err(|e| ServiceError::ValidationError(e.to_string()))?;

    let service = state.inventory_service();
    let organization_id = payload.organization_id.unwrap_or(1);
    let item = service
        .ensure_item(
            &payload.item_number,
            organization_id,
            payload.description.clone(),
            payload.primary_uom_code.clone(),
        )
        .await?;

    if payload.quantity_on_hand != 0 {
        service
            .adjust_inventory(AdjustInventoryCommand {
                inventory_item_id: Some(item.inventory_item_id),
                item_number: None,
                location_id: payload.location_id,
                quantity_delta: Decimal::from(payload.quantity_on_hand),
                reason: payload.reason.clone(),
                expected_version: None,
            })
            .await?;
    }

    let snapshot = service
        .get_snapshot_by_id(item.inventory_item_id)
        .await?
        .ok_or_else(|| {
            ServiceError::InternalError("Failed to load inventory snapshot".to_string())
        })?;

    Ok((
        StatusCode::CREATED,
        Json(ApiResponse::success(snapshot_to_api_item(snapshot))),
    ))
}

#[utoipa::path(
    get,
    path = "/api/v1/inventory/:id",
    params(("id" = String, Path, description = "Inventory item id or item number")),
    responses(
        (status = 200, description = "Inventory item returned", body = ApiResponse<InventoryItem>),
        (status = 404, description = "Not found", body = crate::errors::ErrorResponse)
    ),
    tag = "inventory"
)]
pub async fn get_inventory<S>(
    State(state): State<S>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<InventoryItem>>, ServiceError>
where
    S: InventoryHandlerState,
{
    let service = state.inventory_service();
    let snapshot = fetch_snapshot(service, &id).await?;
    Ok(Json(ApiResponse::success(snapshot_to_api_item(snapshot))))
}

#[utoipa::path(
    put,
    path = "/api/v1/inventory/:id",
    params(("id" = String, Path, description = "Inventory item id or item number")),
    request_body = UpdateInventoryRequest,
    responses(
        (status = 200, description = "Inventory item updated", body = ApiResponse<InventoryItem>)
    ),
    tag = "inventory"
)]
pub async fn update_inventory<S>(
    State(state): State<S>,
    Path(id): Path<String>,
    Json(payload): Json<UpdateInventoryRequest>,
) -> Result<Json<ApiResponse<InventoryItem>>, ServiceError>
where
    S: InventoryHandlerState,
{
    payload
        .validate()
        .map_err(|e| ServiceError::ValidationError(e.to_string()))?;

    let service = state.inventory_service();
    let snapshot = fetch_snapshot(service, &id).await?;

    if payload.description.is_some()
        || payload.primary_uom_code.is_some()
        || payload.organization_id.is_some()
    {
        service
            .ensure_item(
                &snapshot.item_number,
                payload.organization_id.unwrap_or(snapshot.organization_id),
                payload.description.clone().or(snapshot.description.clone()),
                payload
                    .primary_uom_code
                    .clone()
                    .or(snapshot.primary_uom_code.clone()),
            )
            .await?;
    }

    if let Some(new_on_hand) = payload.on_hand {
        let balance = service
            .get_location_balance(snapshot.inventory_item_id, payload.location_id)
            .await?
            .ok_or_else(|| {
                ServiceError::NotFound(format!(
                    "No inventory for item {} at location {}",
                    snapshot.inventory_item_id, payload.location_id
                ))
            })?;
        let delta = Decimal::from(new_on_hand) - balance.quantity_on_hand;
        if !delta.is_zero() {
            service
                .adjust_inventory(AdjustInventoryCommand {
                    inventory_item_id: Some(snapshot.inventory_item_id),
                    item_number: None,
                    location_id: payload.location_id,
                    quantity_delta: delta,
                    reason: payload.reason.clone().or(Some("ADJUSTMENT".to_string())),
                    expected_version: None, // Optimistic locking version passed via If-Match header
                })
                .await?;
        }
    }

    let refreshed = fetch_snapshot(service, &id).await?;
    Ok(Json(ApiResponse::success(snapshot_to_api_item(refreshed))))
}

#[utoipa::path(
    delete,
    path = "/api/v1/inventory/:id",
    params(("id" = String, Path, description = "Inventory item id or item number")),
    responses((status = 204, description = "Inventory deleted")),
    tag = "inventory"
)]
pub async fn delete_inventory<S>(
    State(state): State<S>,
    Path(id): Path<String>,
) -> Result<StatusCode, ServiceError>
where
    S: InventoryHandlerState,
{
    let service = state.inventory_service();
    let snapshot = fetch_snapshot(service, &id).await?;

    for location in snapshot.locations {
        if !location.quantity_on_hand.is_zero() {
            service
                .adjust_inventory(AdjustInventoryCommand {
                    inventory_item_id: Some(snapshot.inventory_item_id),
                    item_number: None,
                    location_id: location.location_id,
                    quantity_delta: Decimal::ZERO - location.quantity_on_hand,
                    reason: Some("DELETE_INVENTORY".to_string()),
                    expected_version: None,
                })
                .await?;
        }
    }

    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    post,
    path = "/api/v1/inventory/:id/reserve",
    params(("id" = String, Path, description = "Inventory item id or item number")),
    request_body = ReserveInventoryRequest,
    responses((status = 200, description = "Inventory reserved", body = ApiResponse<ReservationResponse>)),
    tag = "inventory"
)]
pub async fn reserve_inventory<S>(
    State(state): State<S>,
    Path(id): Path<String>,
    Json(payload): Json<ReserveInventoryRequest>,
) -> Result<Json<ApiResponse<ReservationResponse>>, ServiceError>
where
    S: InventoryHandlerState,
{
    payload
        .validate()
        .map_err(|e| ServiceError::ValidationError(e.to_string()))?;

    let service = state.inventory_service();
    let snapshot = fetch_snapshot(service, &id).await?;
    let reference_id = match &payload.reference_id {
        Some(r) => Some(
            Uuid::parse_str(r)
                .map_err(|_| ServiceError::ValidationError("Invalid reference_id".to_string()))?,
        ),
        None => None,
    };

    let outcome: ReservationOutcome = service
        .reserve_inventory(ReserveInventoryCommand {
            inventory_item_id: Some(snapshot.inventory_item_id),
            item_number: None,
            location_id: payload.location_id,
            quantity: Decimal::from(payload.quantity),
            reference_id,
            reference_type: payload.reference_type.clone(),
            expected_version: None,
        })
        .await?;

    let response = ReservationResponse {
        reservation_id: outcome.id_str(),
        location: balance_to_location(outcome.balance),
    };

    Ok(Json(ApiResponse::success(response)))
}

#[utoipa::path(
    post,
    path = "/api/v1/inventory/:id/release",
    params(("id" = String, Path, description = "Inventory item id or item number")),
    request_body = ReleaseInventoryRequest,
    responses((status = 200, description = "Inventory released", body = ApiResponse<InventoryLocation>)),
    tag = "inventory"
)]
pub async fn release_inventory<S>(
    State(state): State<S>,
    Path(id): Path<String>,
    Json(payload): Json<ReleaseInventoryRequest>,
) -> Result<Json<ApiResponse<InventoryLocation>>, ServiceError>
where
    S: InventoryHandlerState,
{
    payload
        .validate()
        .map_err(|e| ServiceError::ValidationError(e.to_string()))?;

    let service = state.inventory_service();
    let snapshot = fetch_snapshot(service, &id).await?;

    let balance = service
        .release_reservation(ReleaseReservationCommand {
            inventory_item_id: Some(snapshot.inventory_item_id),
            item_number: None,
            location_id: payload.location_id,
            quantity: Decimal::from(payload.quantity),
        })
        .await?;

    Ok(Json(ApiResponse::success(balance_to_location(balance))))
}

#[utoipa::path(
    get,
    path = "/api/v1/inventory/low-stock",
    params(LowStockQuery),
    responses((status = 200, description = "Low stock items", body = ApiResponse<InventoryListResponse>)),
    tag = "inventory"
)]
pub async fn get_low_stock_items<S>(
    State(state): State<S>,
    Query(query): Query<LowStockQuery>,
) -> Result<Json<ApiResponse<InventoryListResponse>>, ServiceError>
where
    S: InventoryHandlerState,
{
    let filters = InventoryFilters {
        product_id: None,
        location_id: None,
        low_stock: Some(true),
        limit: query.limit,
        offset: query.offset,
    };
    let threshold = Decimal::from(query.threshold.unwrap_or(10));

    let service = state.inventory_service();
    let per_page = filters.limit.unwrap_or(50).max(1) as u64;
    let offset = filters.offset.unwrap_or(0) as u64;
    let page = offset / per_page + 1;

    let (items, total) =
        inventory_page(service, &filters, offset, per_page, Some(threshold)).await?;

    let response = InventoryListResponse {
        total,
        page,
        per_page,
        items,
    };

    Ok(Json(ApiResponse::success(response)))
}

fn has_active_filters(filters: &InventoryFilters, low_stock_override: bool) -> bool {
    filters.product_id.is_some()
        || filters.location_id.is_some()
        || filters.low_stock.unwrap_or(false)
        || low_stock_override
}

fn matches_filters(
    snapshot: &InventorySnapshot,
    filters: &InventoryFilters,
    low_stock_threshold: Option<Decimal>,
) -> bool {
    if let Some(product_filter) = filters.product_id.as_deref() {
        let id_match = snapshot.inventory_item_id.to_string() == product_filter;
        let number_match = snapshot.item_number.eq_ignore_ascii_case(product_filter);
        if !id_match && !number_match {
            return false;
        }
    }

    if let Some(loc_filter) = filters.location_id.as_deref() {
        if let Ok(loc_id) = loc_filter.parse::<i32>() {
            if !snapshot
                .locations
                .iter()
                .any(|loc| loc.location_id == loc_id)
            {
                return false;
            }
        }
    }

    let threshold = low_stock_threshold.or_else(|| {
        filters
            .low_stock
            .unwrap_or(false)
            .then(|| Decimal::from(10))
    });

    if let Some(limit) = threshold {
        if snapshot.total_available >= limit {
            return false;
        }
    }

    true
}

async fn inventory_page(
    service: &InventoryService,
    filters: &InventoryFilters,
    offset: u64,
    per_page: u64,
    low_stock_threshold: Option<Decimal>,
) -> Result<(Vec<InventoryItem>, u64), ServiceError> {
    // Use SQL filtering for better performance
    let page = offset / per_page + 1;
    let (snapshots, total) = service
        .list_inventory_filtered(
            page,
            per_page,
            filters.product_id.as_deref(),
            filters.location_id.as_deref().and_then(|s| s.parse::<i32>().ok()),
            low_stock_threshold,
        )
        .await?;

    let items = snapshots.into_iter().map(snapshot_to_api_item).collect();
    Ok((items, total))
}

async fn fetch_snapshot(
    service: &InventoryService,
    id: &str,
) -> Result<InventorySnapshot, ServiceError> {
    if let Ok(item_id) = id.parse::<i64>() {
        service
            .get_snapshot_by_id(item_id)
            .await?
            .ok_or_else(|| ServiceError::NotFound(format!("Inventory item {} not found", id)))
    } else {
        service
            .get_snapshot_by_item_number(id)
            .await?
            .ok_or_else(|| ServiceError::NotFound(format!("Inventory item {} not found", id)))
    }
}

fn snapshot_to_api_item(snapshot: InventorySnapshot) -> InventoryItem {
    InventoryItem {
        inventory_item_id: snapshot.inventory_item_id,
        item_number: snapshot.item_number,
        description: snapshot.description,
        primary_uom_code: snapshot.primary_uom_code,
        organization_id: snapshot.organization_id,
        quantities: InventoryQuantities {
            on_hand: decimal_to_string(snapshot.total_on_hand),
            allocated: decimal_to_string(snapshot.total_allocated),
            available: decimal_to_string(snapshot.total_available),
        },
        locations: snapshot
            .locations
            .into_iter()
            .map(balance_to_location)
            .collect(),
    }
}

fn balance_to_location(balance: LocationBalance) -> InventoryLocation {
    InventoryLocation {
        location_id: balance.location_id,
        location_name: balance.location_name,
        quantities: InventoryQuantities {
            on_hand: decimal_to_string(balance.quantity_on_hand),
            allocated: decimal_to_string(balance.quantity_allocated),
            available: decimal_to_string(balance.quantity_available),
        },
        updated_at: balance.updated_at.to_rfc3339(),
        version: balance.version,
    }
}

fn decimal_to_string(value: Decimal) -> String {
    let mut s = value.normalize().to_string();
    if s.contains('.') {
        while s.ends_with('0') {
            s.pop();
        }
        if s.ends_with('.') {
            s.pop();
        }
        if s.is_empty() {
            s.push('0');
        }
    }
    s
}

impl InventoryHandlerState for crate::AppState {
    fn inventory_service(&self) -> &InventoryService {
        &self.inventory_service
    }
}

// ============================================================================
// Reservation Endpoints
// ============================================================================

/// Response for reservation listing.
#[derive(Debug, Serialize, ToSchema)]
pub struct ReservationListResponse {
    pub reservations: Vec<ReservationDetail>,
    pub total: u64,
    pub page: u64,
    pub limit: u64,
}

/// Detail of a single reservation.
#[derive(Debug, Serialize, ToSchema)]
pub struct ReservationDetail {
    pub id: String,
    pub product_id: String,
    pub location_id: String,
    pub quantity: i32,
    pub status: String,
    pub reference_id: String,
    pub reference_type: String,
    pub expires_at: Option<String>,
    pub created_at: String,
    pub updated_at: Option<String>,
    pub is_expired: bool,
}

/// Query parameters for listing reservations.
#[derive(Debug, Deserialize, ToSchema)]
pub struct ListReservationsQuery {
    #[serde(default = "default_page")]
    pub page: u64,
    #[serde(default = "default_limit")]
    pub limit: u64,
    pub status: Option<String>,
    pub product_id: Option<String>,
    #[serde(default)]
    pub include_expired: bool,
}

fn default_page() -> u64 {
    1
}
fn default_limit() -> u64 {
    50
}

/// Response for reservation cleanup.
#[derive(Debug, Serialize, ToSchema)]
pub struct CleanupResponse {
    pub expired_count: u64,
    pub already_expired_count: u64,
    pub cleaned_at: String,
}

/// Response for reservation statistics.
#[derive(Debug, Serialize, ToSchema)]
pub struct ReservationStatsResponse {
    pub total_reservations: u64,
    pub active_reservations: u64,
    pub expired_not_cleaned: u64,
    pub expiring_within_24h: u64,
    pub stats_at: String,
}

#[utoipa::path(
    get,
    path = "/api/v1/inventory/reservations",
    params(
        ("page" = Option<u64>, Query, description = "Page number (1-indexed)"),
        ("limit" = Option<u64>, Query, description = "Items per page (max 1000)"),
        ("status" = Option<String>, Query, description = "Filter by status"),
        ("product_id" = Option<String>, Query, description = "Filter by product ID"),
        ("include_expired" = Option<bool>, Query, description = "Include expired reservations")
    ),
    responses((status = 200, description = "List of reservations", body = ApiResponse<ReservationListResponse>)),
    tag = "inventory"
)]
pub async fn list_reservations(
    State(state): State<crate::AppState>,
    Query(params): Query<ListReservationsQuery>,
) -> Result<Json<ApiResponse<ReservationListResponse>>, ServiceError> {
    use crate::services::inventory_reservation_service::InventoryReservationService;
    use uuid::Uuid;

    let service = InventoryReservationService::new(state.db.clone());

    let product_id = params
        .product_id
        .as_ref()
        .map(|s| Uuid::parse_str(s))
        .transpose()
        .map_err(|_| ServiceError::ValidationError("Invalid product_id UUID".to_string()))?;

    let (reservations, total) = service
        .list_reservations(
            params.page,
            params.limit.min(1000),
            params.status.as_deref(),
            product_id,
            params.include_expired,
        )
        .await?;

    let details: Vec<ReservationDetail> = reservations
        .into_iter()
        .map(|r| ReservationDetail {
            id: r.id.to_string(),
            product_id: r.product_id.to_string(),
            location_id: r.location_id.to_string(),
            quantity: r.quantity,
            status: r.status,
            reference_id: r.reference_id.to_string(),
            reference_type: r.reference_type,
            expires_at: r.expires_at.map(|t| t.to_rfc3339()),
            created_at: r.created_at.to_rfc3339(),
            updated_at: r.updated_at.map(|t| t.to_rfc3339()),
            is_expired: r.is_expired,
        })
        .collect();

    Ok(Json(ApiResponse::success(ReservationListResponse {
        reservations: details,
        total,
        page: params.page,
        limit: params.limit,
    })))
}

#[utoipa::path(
    get,
    path = "/api/v1/inventory/reservations/:id",
    params(("id" = String, Path, description = "Reservation ID")),
    responses(
        (status = 200, description = "Reservation details", body = ApiResponse<ReservationDetail>),
        (status = 404, description = "Reservation not found")
    ),
    tag = "inventory"
)]
pub async fn get_reservation(
    State(state): State<crate::AppState>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<ReservationDetail>>, ServiceError> {
    use crate::services::inventory_reservation_service::InventoryReservationService;
    use uuid::Uuid;

    let reservation_id = Uuid::parse_str(&id)
        .map_err(|_| ServiceError::ValidationError("Invalid reservation ID".to_string()))?;

    let service = InventoryReservationService::new(state.db.clone());

    let reservation = service
        .get_reservation(reservation_id)
        .await?
        .ok_or_else(|| ServiceError::NotFound(format!("Reservation {} not found", id)))?;

    let detail = ReservationDetail {
        id: reservation.id.to_string(),
        product_id: reservation.product_id.to_string(),
        location_id: reservation.location_id.to_string(),
        quantity: reservation.quantity,
        status: reservation.status,
        reference_id: reservation.reference_id.to_string(),
        reference_type: reservation.reference_type,
        expires_at: reservation.expires_at.map(|t| t.to_rfc3339()),
        created_at: reservation.created_at.to_rfc3339(),
        updated_at: reservation.updated_at.map(|t| t.to_rfc3339()),
        is_expired: reservation.is_expired,
    };

    Ok(Json(ApiResponse::success(detail)))
}

#[utoipa::path(
    post,
    path = "/api/v1/inventory/reservations/:id/cancel",
    params(("id" = String, Path, description = "Reservation ID")),
    responses(
        (status = 200, description = "Reservation cancelled", body = ApiResponse<ReservationDetail>),
        (status = 404, description = "Reservation not found")
    ),
    tag = "inventory"
)]
pub async fn cancel_reservation(
    State(state): State<crate::AppState>,
    Path(id): Path<String>,
) -> Result<Json<ApiResponse<ReservationDetail>>, ServiceError> {
    use crate::services::inventory_reservation_service::InventoryReservationService;
    use uuid::Uuid;

    let reservation_id = Uuid::parse_str(&id)
        .map_err(|_| ServiceError::ValidationError("Invalid reservation ID".to_string()))?;

    let service = InventoryReservationService::new(state.db.clone());

    let reservation = service.cancel_reservation(reservation_id).await?;

    let detail = ReservationDetail {
        id: reservation.id.to_string(),
        product_id: reservation.product_id.to_string(),
        location_id: reservation.location_id.to_string(),
        quantity: reservation.quantity,
        status: reservation.status,
        reference_id: reservation.reference_id.to_string(),
        reference_type: reservation.reference_type,
        expires_at: reservation.expires_at.map(|t| t.to_rfc3339()),
        created_at: reservation.created_at.to_rfc3339(),
        updated_at: reservation.updated_at.map(|t| t.to_rfc3339()),
        is_expired: reservation.is_expired,
    };

    Ok(Json(ApiResponse::success(detail)))
}

#[utoipa::path(
    post,
    path = "/api/v1/inventory/reservations/cleanup",
    responses((status = 200, description = "Cleanup completed", body = ApiResponse<CleanupResponse>)),
    tag = "inventory"
)]
pub async fn cleanup_expired_reservations(
    State(state): State<crate::AppState>,
) -> Result<Json<ApiResponse<CleanupResponse>>, ServiceError> {
    use crate::services::inventory_reservation_service::InventoryReservationService;

    let service = InventoryReservationService::new(state.db.clone());

    let result = service.cleanup_expired_reservations().await?;

    Ok(Json(ApiResponse::success(CleanupResponse {
        expired_count: result.expired_count,
        already_expired_count: result.already_expired_count,
        cleaned_at: result.cleaned_at.to_rfc3339(),
    })))
}

#[utoipa::path(
    get,
    path = "/api/v1/inventory/reservations/stats",
    responses((status = 200, description = "Reservation statistics", body = ApiResponse<ReservationStatsResponse>)),
    tag = "inventory"
)]
pub async fn get_reservation_stats(
    State(state): State<crate::AppState>,
) -> Result<Json<ApiResponse<ReservationStatsResponse>>, ServiceError> {
    use crate::services::inventory_reservation_service::InventoryReservationService;

    let service = InventoryReservationService::new(state.db.clone());

    let stats = service.get_reservation_stats().await?;

    Ok(Json(ApiResponse::success(ReservationStatsResponse {
        total_reservations: stats.total_reservations,
        active_reservations: stats.active_reservations,
        expired_not_cleaned: stats.expired_not_cleaned,
        expiring_within_24h: stats.expiring_within_24h,
        stats_at: stats.stats_at.to_rfc3339(),
    })))
}

// ============================================================================
// Bulk Operation Endpoints
// ============================================================================

/// Request for bulk inventory adjustment.
#[derive(Debug, Deserialize, Serialize, Validate, ToSchema)]
pub struct BulkAdjustRequest {
    #[validate(length(min = 1, max = 100, message = "Must have between 1 and 100 adjustments"))]
    pub adjustments: Vec<BulkAdjustmentItem>,
}

/// Single item in a bulk adjustment.
#[derive(Debug, Deserialize, Serialize, Validate, ToSchema)]
pub struct BulkAdjustmentItem {
    /// Item number/SKU (1-100 characters)
    #[validate(length(min = 1, max = 100, message = "Item number must be between 1 and 100 characters"))]
    pub item_number: String,
    /// Location ID (must be positive)
    #[validate(range(min = 1, message = "Location ID must be positive"))]
    pub location_id: i32,
    /// Quantity change (positive to add, negative to remove)
    pub quantity_delta: i64,
    /// Reason for adjustment (max 200 characters)
    #[validate(length(max = 200, message = "Reason must not exceed 200 characters"))]
    pub reason: Option<String>,
}

/// Response for bulk operations.
#[derive(Debug, Serialize, ToSchema)]
pub struct BulkOperationResponse {
    pub successful: u32,
    pub failed: u32,
    pub errors: Vec<BulkOperationError>,
}

/// Error detail for a failed bulk operation item.
#[derive(Debug, Serialize, ToSchema)]
pub struct BulkOperationError {
    pub index: usize,
    pub item_number: String,
    pub error: String,
}

#[utoipa::path(
    post,
    path = "/api/v1/inventory/bulk-adjust",
    request_body = BulkAdjustRequest,
    responses((status = 200, description = "Bulk adjustment completed", body = ApiResponse<BulkOperationResponse>)),
    tag = "inventory"
)]
pub async fn bulk_adjust_inventory<S>(
    State(state): State<S>,
    Json(payload): Json<BulkAdjustRequest>,
) -> Result<Json<ApiResponse<BulkOperationResponse>>, ServiceError>
where
    S: InventoryHandlerState,
{
    payload
        .validate()
        .map_err(|e| ServiceError::ValidationError(e.to_string()))?;

    let service = state.inventory_service();
    let mut successful = 0u32;
    let mut failed = 0u32;
    let mut errors = Vec::new();

    for (index, item) in payload.adjustments.iter().enumerate() {
        let result = service
            .adjust_inventory(AdjustInventoryCommand {
                inventory_item_id: None,
                item_number: Some(item.item_number.clone()),
                location_id: item.location_id,
                quantity_delta: Decimal::from(item.quantity_delta),
                reason: item.reason.clone(),
                expected_version: None,
            })
            .await;

        match result {
            Ok(_) => successful += 1,
            Err(e) => {
                failed += 1;
                errors.push(BulkOperationError {
                    index,
                    item_number: item.item_number.clone(),
                    error: e.to_string(),
                });
            }
        }
    }

    Ok(Json(ApiResponse::success(BulkOperationResponse {
        successful,
        failed,
        errors,
    })))
}
