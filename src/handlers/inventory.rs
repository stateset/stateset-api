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
pub struct InventoryItem {
    pub inventory_item_id: i64,
    pub item_number: String,
    pub description: Option<String>,
    pub primary_uom_code: Option<String>,
    pub organization_id: i64,
    pub quantities: InventoryQuantities,
    pub locations: Vec<InventoryLocation>,
}

/// API representation of quantities.
#[derive(Debug, Serialize, ToSchema)]
pub struct InventoryQuantities {
    pub on_hand: String,
    pub allocated: String,
    pub available: String,
}

/// API representation of inventory at a specific location.
#[derive(Debug, Serialize, ToSchema)]
pub struct InventoryLocation {
    pub location_id: i32,
    pub location_name: Option<String>,
    pub quantities: InventoryQuantities,
    pub updated_at: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct InventoryListResponse {
    pub items: Vec<InventoryItem>,
    pub total: u64,
    pub page: u64,
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
pub struct CreateInventoryRequest {
    #[validate(length(min = 1))]
    pub item_number: String,
    pub description: Option<String>,
    pub primary_uom_code: Option<String>,
    pub organization_id: Option<i64>,
    pub location_id: i32,
    pub quantity_on_hand: i64,
    pub reason: Option<String>,
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct UpdateInventoryRequest {
    pub location_id: i32,
    pub on_hand: Option<i64>,
    pub description: Option<String>,
    pub primary_uom_code: Option<String>,
    pub organization_id: Option<i64>,
    pub reason: Option<String>,
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct ReserveInventoryRequest {
    pub location_id: i32,
    #[validate(range(min = 1))]
    pub quantity: i64,
    pub reference_id: Option<String>,
    pub reference_type: Option<String>,
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct ReleaseInventoryRequest {
    pub location_id: i32,
    #[validate(range(min = 1))]
    pub quantity: i64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ReservationResponse {
    pub reservation_id: String,
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
        (status = 200, description = "Inventory list returned", body = ApiResponse<InventoryListResponse>)
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
        (status = 201, description = "Inventory created", body = ApiResponse<InventoryItem>)
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
    path = "/api/v1/inventory/{id}",
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
    path = "/api/v1/inventory/{id}",
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
                })
                .await?;
        }
    }

    let refreshed = fetch_snapshot(service, &id).await?;
    Ok(Json(ApiResponse::success(snapshot_to_api_item(refreshed))))
}

#[utoipa::path(
    delete,
    path = "/api/v1/inventory/{id}",
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
                })
                .await?;
        }
    }

    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    post,
    path = "/api/v1/inventory/{id}/reserve",
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
    path = "/api/v1/inventory/{id}/release",
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
    if !has_active_filters(filters, low_stock_threshold.is_some()) {
        let page = offset / per_page + 1;
        let (snapshots, total) = service.list_inventory(page, per_page).await?;
        let items = snapshots.into_iter().map(snapshot_to_api_item).collect();
        return Ok((items, total));
    }

    let fetch_page_size = per_page.max(100).min(1000);
    let mut page_index = 1_u64;
    let mut filtered_total = 0_u64;
    let mut collected: Vec<InventorySnapshot> = Vec::new();

    loop {
        let (snapshots, total) = service.list_inventory(page_index, fetch_page_size).await?;

        if snapshots.is_empty() {
            break;
        }

        for snapshot in snapshots {
            if matches_filters(&snapshot, filters, low_stock_threshold.clone()) {
                if filtered_total >= offset && collected.len() < per_page as usize {
                    collected.push(snapshot);
                }
                filtered_total += 1;
            }
        }

        if page_index * fetch_page_size >= total {
            break;
        }
        page_index += 1;
    }

    let items = collected.into_iter().map(snapshot_to_api_item).collect();

    Ok((items, filtered_total))
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
