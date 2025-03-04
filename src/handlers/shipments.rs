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
    services::shipments::ShipmentService,
    commands::shipments::{
        create_shipment_command::CreateShipmentCommand,
        update_shipment_command::UpdateShipmentCommand,
        cancel_shipment_command::CancelShipmentCommand,
        track_shipment_command::TrackShipmentCommand,
        confirm_shipment_delivery_command::ConfirmShipmentDeliveryCommand,
        assign_shipment_carrier_command::AssignShipmentCarrierCommand,
        ship_command::ShipCommand,
    },
    AppState,
};
use serde::{Serialize, Deserialize};
use validator::Validate;
use tracing::info;
use super::common::{validate_input, map_service_error, success_response, created_response, no_content_response, PaginationParams};

// Add routes configuration
pub fn shipments_routes() -> Router {
    Router::new()
        .route("/", get(list_shipments))
        .route("/:id", get(get_shipment))
        .route("/", post(create_shipment))
        .route("/:id", put(update_shipment))
        .route("/:id/cancel", post(cancel_shipment))
        .route("/:id/track", get(track_shipment))
        .route("/:id/confirm-delivery", post(confirm_delivery))
        .route("/:id/assign-carrier", post(assign_carrier))
        .route("/:id/ship", post(ship))
        .route("/order/:order_id", get(get_shipments_for_order))
}

/// List shipments with optional filtering and pagination
async fn list_shipments(
    State(state): State<Arc<AppState>>,
    AuthenticatedUser(_user): AuthenticatedUser,
    Query(params): Query<PaginationParams>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    let (shipments, total) = state.services.shipments
        .list_shipments(params.page.unwrap_or(1), params.limit.unwrap_or(20))
        .await
        .map_err(map_service_error)?;
    
    success_response(serde_json::json!({
        "shipments": shipments,
        "total": total,
        "page": params.page.unwrap_or(1),
        "limit": params.limit.unwrap_or(20)
    }))
}

// Request and response DTOs

#[derive(Debug, Deserialize, Validate)]
pub struct CreateShipmentRequest {
    pub order_id: Uuid,
    #[validate(length(min = 1, message = "Recipient name cannot be empty"))]
    pub recipient_name: String,
    #[validate(length(min = 1, message = "Shipping address cannot be empty"))]
    pub shipping_address: String,
    pub carrier: Option<String>,
    pub tracking_number: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateShipmentRequest {
    pub recipient_name: Option<String>,
    pub shipping_address: Option<String>,
    pub carrier: Option<String>,
    pub tracking_number: Option<String>,
    pub status: Option<String>,
    pub estimated_delivery_date: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CancelShipmentRequest {
    #[validate(length(min = 1, message = "Reason cannot be empty"))]
    pub reason: String,
}

#[derive(Debug, Deserialize, Validate)]
pub struct ConfirmDeliveryRequest {
    pub delivered_to: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct AssignCarrierRequest {
    #[validate(length(min = 1, message = "Carrier cannot be empty"))]
    pub carrier: String,
    pub service_level: Option<String>,
    pub tracking_number: Option<String>,
    pub estimated_delivery_date: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct ShipRequest {
    pub shipped_by: Uuid,
    pub tracking_number: Option<String>,
    pub notes: Option<String>,
}

// Handler functions

/// Create a new shipment
async fn create_shipment(
    State(state): State<Arc<AppState>>,
    AuthenticatedUser(user): AuthenticatedUser,
    Json(payload): Json<CreateShipmentRequest>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    validate_input(&payload)?;
    
    let command = CreateShipmentCommand {
        order_id: payload.order_id,
        recipient_name: payload.recipient_name,
        shipping_address: payload.shipping_address,
        carrier: payload.carrier,
        tracking_number: payload.tracking_number,
    };
    
    let shipment_id = state.services.shipments
        .create_shipment(command)
        .await
        .map_err(map_service_error)?;
    
    info\!("Shipment created: {}", shipment_id);
    
    created_response(serde_json::json\!({
        "id": shipment_id,
        "message": "Shipment created successfully"
    }))
}

/// Get a shipment by ID
async fn get_shipment(
    State(state): State<Arc<AppState>>,
    Path(shipment_id): Path<Uuid>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    let shipment = state.services.shipments
        .get_shipment(&shipment_id)
        .await
        .map_err(map_service_error)?
        .ok_or_else(|| ApiError::NotFound(format\!("Shipment with ID {} not found", shipment_id)))?;
    
    success_response(shipment)
}

/// Update a shipment
async fn update_shipment(
    State(state): State<Arc<AppState>>,
    Path(shipment_id): Path<Uuid>,
    Json(payload): Json<UpdateShipmentRequest>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    validate_input(&payload)?;
    
    // Parse the estimated delivery date if provided
    let estimated_delivery_date = match payload.estimated_delivery_date {
        Some(date_str) => {
            Some(chrono::NaiveDate::parse_from_str(&date_str, "%Y-%m-%d")
                .map_err(|e| ApiError::BadRequest(format\!("Invalid date format: {}", e)))?
                .and_hms_opt(0, 0, 0)
                .unwrap())
        },
        None => None,
    };
    
    let command = UpdateShipmentCommand {
        id: shipment_id,
        recipient_name: payload.recipient_name,
        shipping_address: payload.shipping_address,
        carrier: payload.carrier,
        tracking_number: payload.tracking_number,
        status: payload.status,
        estimated_delivery_date,
    };
    
    state.services.shipments
        .update_shipment(command)
        .await
        .map_err(map_service_error)?;
    
    info\!("Shipment updated: {}", shipment_id);
    
    success_response(serde_json::json\!({
        "message": "Shipment updated successfully"
    }))
}

/// Cancel a shipment
async fn cancel_shipment(
    State(state): State<Arc<AppState>>,
    Path(shipment_id): Path<Uuid>,
    Json(payload): Json<CancelShipmentRequest>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    validate_input(&payload)?;
    
    let command = CancelShipmentCommand {
        id: shipment_id,
        reason: payload.reason,
    };
    
    state.services.shipments
        .cancel_shipment(command)
        .await
        .map_err(map_service_error)?;
    
    info\!("Shipment cancelled: {}", shipment_id);
    
    success_response(serde_json::json\!({
        "message": "Shipment cancelled successfully"
    }))
}

/// Track a shipment
async fn track_shipment(
    State(state): State<Arc<AppState>>,
    Path(shipment_id): Path<Uuid>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    let command = TrackShipmentCommand {
        id: shipment_id,
    };
    
    let tracking_status = state.services.shipments
        .track_shipment(command)
        .await
        .map_err(map_service_error)?;
    
    info\!("Shipment tracked: {}", shipment_id);
    
    success_response(serde_json::json\!({
        "shipment_id": shipment_id,
        "tracking_status": tracking_status
    }))
}

/// Confirm delivery of a shipment
async fn confirm_delivery(
    State(state): State<Arc<AppState>>,
    Path(shipment_id): Path<Uuid>,
    Json(payload): Json<ConfirmDeliveryRequest>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    validate_input(&payload)?;
    
    let command = ConfirmShipmentDeliveryCommand {
        id: shipment_id,
        delivered_to: payload.delivered_to,
        notes: payload.notes,
    };
    
    state.services.shipments
        .confirm_delivery(command)
        .await
        .map_err(map_service_error)?;
    
    info\!("Shipment delivery confirmed: {}", shipment_id);
    
    success_response(serde_json::json\!({
        "message": "Shipment delivery confirmed successfully"
    }))
}

/// Assign a carrier to a shipment
async fn assign_carrier(
    State(state): State<Arc<AppState>>,
    Path(shipment_id): Path<Uuid>,
    Json(payload): Json<AssignCarrierRequest>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    validate_input(&payload)?;
    
    // Parse the estimated delivery date if provided
    let estimated_delivery_date = match payload.estimated_delivery_date {
        Some(date_str) => {
            Some(chrono::NaiveDate::parse_from_str(&date_str, "%Y-%m-%d")
                .map_err(|e| ApiError::BadRequest(format\!("Invalid date format: {}", e)))?
                .and_hms_opt(0, 0, 0)
                .unwrap())
        },
        None => None,
    };
    
    let command = AssignShipmentCarrierCommand {
        id: shipment_id,
        carrier: payload.carrier,
        service_level: payload.service_level,
        tracking_number: payload.tracking_number,
        estimated_delivery_date,
    };
    
    state.services.shipments
        .assign_carrier(command)
        .await
        .map_err(map_service_error)?;
    
    info\!("Carrier assigned to shipment: {}", shipment_id);
    
    success_response(serde_json::json\!({
        "message": "Carrier assigned to shipment successfully"
    }))
}

/// Process the shipping of a shipment
async fn ship(
    State(state): State<Arc<AppState>>,
    Path(shipment_id): Path<Uuid>,
    Json(payload): Json<ShipRequest>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    validate_input(&payload)?;
    
    let command = ShipCommand {
        id: shipment_id,
        shipped_by: payload.shipped_by,
        tracking_number: payload.tracking_number,
        notes: payload.notes,
    };
    
    state.services.shipments
        .ship(command)
        .await
        .map_err(map_service_error)?;
    
    info\!("Shipment shipped: {}", shipment_id);
    
    success_response(serde_json::json\!({
        "message": "Shipment shipped successfully"
    }))
}

/// Get shipments for an order
async fn get_shipments_for_order(
    State(state): State<Arc<AppState>>,
    Path(order_id): Path<Uuid>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    let shipments = state.services.shipments
        .get_shipments_for_order(&order_id)
        .await
        .map_err(map_service_error)?;
    
    success_response(shipments)
}

/// Creates the router for shipment endpoints
pub fn shipments_routes() -> Router {
    Router::new()
        .route("/", post(create_shipment))
        .route("/:id", get(get_shipment))
        .route("/:id", put(update_shipment))
        .route("/:id/cancel", post(cancel_shipment))
        .route("/:id/track", get(track_shipment))
        .route("/:id/confirm-delivery", post(confirm_delivery))
        .route("/:id/assign-carrier", post(assign_carrier))
        .route("/:id/ship", post(ship))
        .route("/order/:order_id", get(get_shipments_for_order))
}
