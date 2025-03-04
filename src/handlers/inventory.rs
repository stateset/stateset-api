use axum::{
    routing::{get, post, put, delete},
    extract::{State, Path, Query, Json},
    Router,
};
use std::sync::Arc;
use uuid::Uuid;
use crate::{
    auth::AuthenticatedUser,
    errors::{ApiError, ServiceError},
    services::inventory::InventoryService,
    commands::inventory::{
        adjust_inventory_command::AdjustInventoryCommand,
        allocate_inventory_command::AllocateInventoryCommand, 
        deallocate_inventory_command::DeallocateInventoryCommand,
        reserve_inventory_command::ReserveInventoryCommand,
        release_inventory_command::ReleaseInventoryCommand,
    },
    main::AppState,
};
use serde::{Serialize, Deserialize};
use validator::Validate;
use tracing::info;
use super::common::{validate_input, map_service_error, success_response, created_response, no_content_response, PaginationParams};

/// Creates the router for inventory endpoints
pub fn inventory_routes() -> Router {
    Router::new()
        .route("/", get(list_inventory))
        .route("/:id", get(get_inventory_item))
        .route("/adjust", post(adjust_inventory))
        .route("/allocate", post(allocate_inventory))
        .route("/deallocate", post(deallocate_inventory))
        .route("/reserve", post(reserve_inventory))
        .route("/reserve/v2", post(reserve_inventory_v2))  // New enhanced reservation endpoint
        .route("/release", post(release_inventory))
        .route("/levels", post(set_inventory_levels))
        .route("/levels/:product_id/:location_id", get(get_inventory_levels))
        .route("/:product_id/:location_id", get(get_inventory))
        .route("/:product_id/:location_id/available", get(check_inventory_availability))
        .route("/transfer", post(transfer_inventory))
        .route("/receive", post(receive_inventory))
        .route("/cycle-count", post(cycle_count))
}

/// List all inventory items with pagination
async fn list_inventory(
    State(state): State<Arc<AppState>>,
    AuthenticatedUser(_user): AuthenticatedUser,
    Query(params): Query<PaginationParams>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    let (items, total) = state.services.inventory
        .list_inventory(params.page.unwrap_or(1), params.limit.unwrap_or(20))
        .await
        .map_err(map_service_error)?;
    
    success_response(serde_json::json!({
        "items": items,
        "total": total,
        "page": params.page.unwrap_or(1),
        "limit": params.limit.unwrap_or(20)
    }))
}

/// Get inventory item by ID
async fn get_inventory_item(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    // For now, we'll use a placeholder implementation
    // In a real implementation, we would fetch the inventory item by ID
    success_response(serde_json::json!({
        "message": "Get inventory endpoint (placeholder)",
        "id": id,
        "quantity": 0
    }))
}

// Request and response DTOs

#[derive(Debug, Deserialize, Validate)]
pub struct AdjustInventoryRequest {
    pub product_id: Uuid,
    pub location_id: Uuid,
    pub adjustment: i32,
    #[validate(length(min = 1, message = "Reason cannot be empty"))]
    pub reason: String,
}

#[derive(Debug, Deserialize, Validate)]
pub struct AllocateInventoryRequest {
    pub product_id: Uuid,
    pub location_id: Uuid,
    #[validate(range(min = 1, message = "Quantity must be at least 1"))]
    pub quantity: i32,
    pub order_id: Uuid,
    pub order_item_id: Option<Uuid>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct DeallocateInventoryRequest {
    pub product_id: Uuid,
    pub location_id: Uuid,
    #[validate(range(min = 1, message = "Quantity must be at least 1"))]
    pub quantity: i32,
    pub order_id: Uuid,
    pub order_item_id: Option<Uuid>,
    pub reason: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct ReserveInventoryRequest {
    pub product_id: Uuid,
    pub location_id: Uuid,
    #[validate(range(min = 1, message = "Quantity must be at least 1"))]
    pub quantity: i32,
    pub reservation_id: Uuid,
    pub reason: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct ReleaseInventoryRequest {
    pub product_id: Uuid,
    pub location_id: Uuid,
    #[validate(range(min = 1, message = "Quantity must be at least 1"))]
    pub quantity: i32,
    pub reservation_id: Uuid,
    pub reason: Option<String>,
}

// Request DTOs
#[derive(Debug, Deserialize, Validate)]
pub struct SetInventoryLevelsRequest {
    pub product_id: Uuid,
    pub location_id: Uuid,
    #[validate(range(min = 0, message = "Quantity cannot be negative"))]
    pub quantity: i32,
    #[validate(range(min = 0, message = "Reserved quantity cannot be negative"))]
    pub reserved: i32,
    #[validate(range(min = 0, message = "Allocated quantity cannot be negative"))]
    pub allocated: i32,
    #[validate(range(min = 0, message = "Available quantity cannot be negative"))]
    pub available: i32,
    #[validate(length(min = 1, message = "Reason cannot be empty"))]
    pub reason: String,
}

// Handler functions

/// Adjust inventory levels
async fn adjust_inventory(
    State(state): State<Arc<AppState>>,
    AuthenticatedUser(user): AuthenticatedUser,
    Json(payload): Json<AdjustInventoryRequest>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    validate_input(&payload)?;
    
    let command = AdjustInventoryCommand {
        product_id: payload.product_id,
        location_id: payload.location_id,
        adjustment: payload.adjustment,
        reason: payload.reason,
    };
    
    state.services.inventory
        .adjust_inventory(command)
        .await
        .map_err(map_service_error)?;
    
    info!("Inventory adjusted for product: {} at location: {}", payload.product_id, payload.location_id);
    
    success_response(serde_json::json!({
        "message": "Inventory adjusted successfully"
    }))
}

/// Set inventory levels
async fn set_inventory_levels(
    State(state): State<Arc<AppState>>,
    AuthenticatedUser(user): AuthenticatedUser,
    Json(payload): Json<SetInventoryLevelsRequest>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    validate_input(&payload)?;
    
    let command = crate::commands::inventory::set_inventory_levels_command::SetInventoryLevelsCommand {
        product_id: payload.product_id,
        location_id: payload.location_id,
        quantity: payload.quantity,
        reserved: payload.reserved,
        allocated: payload.allocated,
        available: payload.available,
        reason: payload.reason,
    };
    
    let result = state.services.inventory
        .set_inventory_levels(command)
        .await
        .map_err(map_service_error)?;
    
    info!("Inventory levels set for product: {} at location: {}", payload.product_id, payload.location_id);
    
    success_response(result)
}

/// Get inventory levels for a product at a location
async fn get_inventory_levels(
    State(state): State<Arc<AppState>>,
    Path((product_id, location_id)): Path<(Uuid, Uuid)>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    let inventory = state.services.inventory
        .get_inventory(&product_id, &location_id)
        .await
        .map_err(map_service_error)?
        .ok_or_else(|| ApiError::NotFound(format!("Inventory not found for product {} at location {}", product_id, location_id)))?;
    
    success_response(inventory)
}

/// Allocate inventory to an order
async fn allocate_inventory(
    State(state): State<Arc<AppState>>,
    AuthenticatedUser(user): AuthenticatedUser,
    Json(payload): Json<AllocateInventoryRequest>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    validate_input(&payload)?;
    
    let command = AllocateInventoryCommand {
        product_id: payload.product_id,
        location_id: payload.location_id,
        quantity: payload.quantity,
        order_id: payload.order_id,
        order_item_id: payload.order_item_id,
    };
    
    state.services.inventory
        .allocate_inventory(command)
        .await
        .map_err(map_service_error)?;
    
    info!("Inventory allocated for product: {} at location: {}", payload.product_id, payload.location_id);
    
    success_response(serde_json::json!({
        "message": "Inventory allocated successfully"
    }))
}

/// Deallocate inventory from an order
async fn deallocate_inventory(
    State(state): State<Arc<AppState>>,
    AuthenticatedUser(user): AuthenticatedUser,
    Json(payload): Json<DeallocateInventoryRequest>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    validate_input(&payload)?;
    
    let command = DeallocateInventoryCommand {
        product_id: payload.product_id,
        location_id: payload.location_id,
        quantity: payload.quantity,
        order_id: payload.order_id,
        order_item_id: payload.order_item_id,
        reason: payload.reason,
    };
    
    state.services.inventory
        .deallocate_inventory(command)
        .await
        .map_err(map_service_error)?;
    
    info!("Inventory deallocated for product: {} at location: {}", payload.product_id, payload.location_id);
    
    success_response(serde_json::json!({
        "message": "Inventory deallocated successfully"
    }))
}

/// Reserve inventory
async fn reserve_inventory(
    State(state): State<Arc<AppState>>,
    AuthenticatedUser(user): AuthenticatedUser,
    Json(payload): Json<ReserveInventoryRequest>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    validate_input(&payload)?;
    
    let command = ReserveInventoryCommand {
        product_id: payload.product_id,
        location_id: payload.location_id,
        quantity: payload.quantity,
        reservation_id: payload.reservation_id,
        reason: payload.reason,
    };
    
    state.services.inventory
        .reserve_inventory(command)
        .await
        .map_err(map_service_error)?;
    
    info!("Inventory reserved for product: {} at location: {}", payload.product_id, payload.location_id);
    
    success_response(serde_json::json!({
        "message": "Inventory reserved successfully"
    }))
}

/// Release reserved inventory
async fn release_inventory(
    State(state): State<Arc<AppState>>,
    AuthenticatedUser(user): AuthenticatedUser,
    Json(payload): Json<ReleaseInventoryRequest>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    validate_input(&payload)?;
    
    let command = ReleaseInventoryCommand {
        product_id: payload.product_id,
        location_id: payload.location_id,
        quantity: payload.quantity,
        reservation_id: payload.reservation_id,
        reason: payload.reason,
    };
    
    state.services.inventory
        .release_inventory(command)
        .await
        .map_err(map_service_error)?;
    
    info!("Inventory released for product: {} at location: {}", payload.product_id, payload.location_id);
    
    success_response(serde_json::json!({
        "message": "Inventory released successfully"
    }))
}

/// Get inventory levels for a product at a location
async fn get_inventory(
    State(state): State<Arc<AppState>>,
    Path((product_id, location_id)): Path<(Uuid, Uuid)>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    let inventory = state.services.inventory
        .get_inventory(&product_id, &location_id)
        .await
        .map_err(map_service_error)?
        .ok_or_else(|| ApiError::NotFound(format!("Inventory not found for product {} at location {}", product_id, location_id)))?;
    
    success_response(inventory)
}

/// Check if a product is in stock at a location
async fn check_inventory_availability(
    State(state): State<Arc<AppState>>,
    Path((product_id, location_id)): Path<(Uuid, Uuid)>,
    Query(params): Query<QuantityQuery>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    let quantity = params.quantity.unwrap_or(1);
    
    let is_available = state.services.inventory
        .is_in_stock(&product_id, &location_id, quantity)
        .await
        .map_err(map_service_error)?;
    
    success_response(serde_json::json!({
        "product_id": product_id,
        "location_id": location_id,
        "quantity": quantity,
        "available": is_available
    }))
}

#[derive(Debug, Deserialize)]
pub struct QuantityQuery {
    pub quantity: Option<i32>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct TransferInventoryRequest {
    pub product_id: Uuid,
    pub from_location_id: Uuid,
    pub to_location_id: Uuid,
    #[validate(range(min = 1, message = "Quantity must be at least 1"))]
    pub quantity: i32,
    pub lot_number: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct ReceiveInventoryRequest {
    pub product_id: Uuid,
    pub location_id: Uuid,
    #[validate(range(min = 1, message = "Quantity must be at least 1"))]
    pub quantity: i32,
    pub lot_number: Option<String>,
    pub expiration_date: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CycleCountItemRequest {
    pub product_id: Uuid,
    pub counted_quantity: i32,
    pub lot_number: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CycleCountRequest {
    pub location_id: Uuid,
    #[validate(length(min = 1, message = "At least one item must be counted"))]
    pub items: Vec<CycleCountItemRequest>,
    pub notes: Option<String>,
    pub counted_by: Uuid,
}

/// Transfer inventory from one location to another
async fn transfer_inventory(
    State(state): State<Arc<AppState>>,
    AuthenticatedUser(user): AuthenticatedUser,
    Json(payload): Json<TransferInventoryRequest>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    validate_input(&payload)?;
    
    let command = crate::commands::inventory::transfer_inventory_command::TransferInventoryCommand {
        product_id: payload.product_id.to_string(),
        from_location_id: payload.from_location_id.to_string(),
        to_location_id: payload.to_location_id.to_string(),
        quantity: payload.quantity,
        lot_number: payload.lot_number,
        notes: payload.notes,
    };
    
    state.services.inventory
        .transfer_inventory(command)
        .await
        .map_err(map_service_error)?;
    
    info!("Inventory transferred from location {} to location {} for product {}", 
          payload.from_location_id, payload.to_location_id, payload.product_id);
    
    success_response(serde_json::json!({
        "message": "Inventory transferred successfully"
    }))
}

/// Receive new inventory into a location
async fn receive_inventory(
    State(state): State<Arc<AppState>>,
    AuthenticatedUser(user): AuthenticatedUser,
    Json(payload): Json<ReceiveInventoryRequest>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    validate_input(&payload)?;
    
    // Parse the expiration date if provided
    let expiration_date = match payload.expiration_date {
        Some(date_str) => {
            Some(chrono::NaiveDate::parse_from_str(&date_str, "%Y-%m-%d")
                .map_err(|e| ApiError::BadRequest(format!("Invalid date format: {}", e)))?
                .and_hms_opt(0, 0, 0)
                .unwrap())
        },
        None => None,
    };
    
    let command = crate::commands::inventory::receive_inventory_command::ReceiveInventoryCommand {
        product_id: payload.product_id.to_string(),
        location_id: payload.location_id.to_string(),
        quantity: payload.quantity,
        lot_number: payload.lot_number,
        expiration_date,
        notes: payload.notes,
    };
    
    state.services.inventory
        .receive_inventory(command)
        .await
        .map_err(map_service_error)?;
    
    info!("Inventory received at location {} for product {}", 
          payload.location_id, payload.product_id);
    
    success_response(serde_json::json!({
        "message": "Inventory received successfully"
    }))
}

/// Perform a cycle count at a location
async fn cycle_count(
    State(state): State<Arc<AppState>>,
    AuthenticatedUser(user): AuthenticatedUser,
    Json(payload): Json<CycleCountRequest>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    validate_input(&payload)?;
    
    // Map the cycle count items
    let items = payload.items
        .into_iter()
        .map(|item| crate::commands::inventory::cycle_count_command::CycleCountItem {
            product_id: item.product_id.to_string(),
            counted_quantity: item.counted_quantity,
            lot_number: item.lot_number,
        })
        .collect();
    
    let command = crate::commands::inventory::cycle_count_command::CycleCountCommand {
        location_id: payload.location_id.to_string(),
        items,
        notes: payload.notes,
        counted_by: payload.counted_by.to_string(),
    };
    
    state.services.inventory
        .cycle_count(command)
        .await
        .map_err(map_service_error)?;
    
    info!("Cycle count performed at location {}", payload.location_id);
    
    success_response(serde_json::json!({
        "message": "Cycle count completed successfully"
    }))
}

#[derive(Debug, Deserialize, Validate)]
pub struct ReserveInventoryV2Request {
    pub warehouse_id: String,
    pub reference_id: Uuid,         // Order ID, Customer ID, etc.
    pub reference_type: String,     // "SALES_ORDER", "CUSTOMER_HOLD", etc.
    #[validate(length(min = 1))]
    pub items: Vec<ReservationItemRequest>,
    pub reservation_type: String,   // "SalesOrder", "CustomerHold", etc.
    #[validate(range(min = 1, max = 365))]
    pub duration_days: Option<i32>, // How long to hold the reservation
    pub priority: Option<i32>,      // Higher priority reservations take precedence
    #[validate(length(max = 500))]
    pub notes: Option<String>,
    pub reservation_strategy: String, // "Strict", "Partial", "WithSubstitutes", "BestEffort"
}

#[derive(Debug, Deserialize, Validate)]
pub struct ReservationItemRequest {
    pub product_id: Uuid,
    #[validate(range(min = 1))]
    pub quantity: i32,
    pub lot_numbers: Option<Vec<String>>,
    pub location_id: Option<String>,
    pub substitutes: Option<Vec<Uuid>>, // Alternative products that can be reserved
}

/// Reserve inventory with enhanced functionality (v2)
async fn reserve_inventory_v2(
    State(state): State<Arc<AppState>>,
    AuthenticatedUser(user): AuthenticatedUser,
    Json(payload): Json<ReserveInventoryV2Request>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    validate_input(&payload)?;
    
    // Convert the reservation strategy string to enum
    let reservation_strategy = match payload.reservation_strategy.as_str() {
        "Strict" => crate::commands::inventory::reserve_inventory_command::ReservationStrategy::Strict,
        "Partial" => crate::commands::inventory::reserve_inventory_command::ReservationStrategy::Partial,
        "WithSubstitutes" => crate::commands::inventory::reserve_inventory_command::ReservationStrategy::WithSubstitutes,
        "BestEffort" => crate::commands::inventory::reserve_inventory_command::ReservationStrategy::BestEffort,
        _ => return Err(ApiError::BadRequest(format!("Invalid reservation strategy: {}", payload.reservation_strategy))),
    };
    
    // Convert the reservation type string to enum
    let reservation_type = match payload.reservation_type.as_str() {
        "SalesOrder" => crate::commands::inventory::reserve_inventory_command::ReservationType::SalesOrder,
        "CustomerHold" => crate::commands::inventory::reserve_inventory_command::ReservationType::CustomerHold,
        "Production" => crate::commands::inventory::reserve_inventory_command::ReservationType::Production,
        "QualityHold" => crate::commands::inventory::reserve_inventory_command::ReservationType::QualityHold,
        "PreOrder" => crate::commands::inventory::reserve_inventory_command::ReservationType::PreOrder,
        "SafetyStock" => crate::commands::inventory::reserve_inventory_command::ReservationType::SafetyStock,
        _ => return Err(ApiError::BadRequest(format!("Invalid reservation type: {}", payload.reservation_type))),
    };
    
    // Convert request items to command items
    let items = payload.items.into_iter()
        .map(|item| crate::commands::inventory::reserve_inventory_command::ReservationRequest {
            product_id: item.product_id,
            quantity: item.quantity,
            lot_numbers: item.lot_numbers,
            location_id: item.location_id,
            substitutes: item.substitutes,
        })
        .collect();
    
    let command = ReserveInventoryCommand {
        warehouse_id: payload.warehouse_id,
        reference_id: payload.reference_id,
        reference_type: payload.reference_type,
        items,
        reservation_type,
        duration_days: payload.duration_days,
        priority: payload.priority,
        notes: payload.notes,
        reservation_strategy,
    };
    
    let result = state.services.inventory
        .reserve_inventory_v2(command)
        .await
        .map_err(map_service_error)?;
    
    info!("Inventory reserved for reference: {} of type: {} with {} items", 
          payload.reference_id, payload.reference_type, result.reservations.len());
    
    success_response(result)
}
