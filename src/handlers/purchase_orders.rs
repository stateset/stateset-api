use super::common::{created_response, map_service_error, success_response, validate_input};
use crate::{
    auth::AuthenticatedUser,
    commands::purchaseorders::{
        approve_purchase_order_command::ApprovePurchaseOrderCommand,
        cancel_purchase_order_command::CancelPurchaseOrderCommand,
        create_purchase_order_command::{
            CreatePurchaseOrderCommand,
            PurchaseOrderItemRequest as CommandPurchaseOrderItemRequest,
            ShippingAddress as CommandShippingAddress,
        },
        receive_purchase_order_command::ReceivePurchaseOrderCommand,
        reject_purchase_order_command::RejectPurchaseOrderCommand,
        submit_purchase_order_command::SubmitPurchaseOrderCommand,
        update_purchase_order_command::UpdatePurchaseOrderCommand,
    },
    errors::ApiError,
    handlers::AppState,
};
use axum::{
    extract::{Json, Path, Query, State},
    routing::{get, post, put},
    Router,
};
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use tracing::info;
use utoipa::ToSchema;
use uuid::Uuid;
use validator::Validate;

// Request and response DTOs

#[derive(Debug, Serialize, Deserialize, Validate, ToSchema)]
pub struct CreatePurchaseOrderRequest {
    pub supplier_id: Uuid,
    #[validate(length(min = 1))]
    pub expected_delivery_date: String,
    #[validate]
    pub shipping_address: ShippingAddressRequest,
    #[validate(length(min = 1))]
    pub items: Vec<PurchaseOrderItemRequest>,
    pub payment_terms: Option<String>,
    pub currency: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Validate, ToSchema)]
pub struct PurchaseOrderItemRequest {
    pub product_id: Uuid,
    #[validate(range(min = 1))]
    pub quantity: i32,
    #[validate(range(min = 0.0))]
    pub unit_price: f64,
    #[validate(range(min = 0.0))]
    pub tax_rate: Option<f64>,
    pub currency: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Validate, ToSchema)]
pub struct ShippingAddressRequest {
    #[validate(length(min = 1))]
    pub street: String,
    #[validate(length(min = 1))]
    pub city: String,
    #[validate(length(min = 1))]
    pub state: String,
    #[validate(length(min = 1))]
    pub postal_code: String,
    #[validate(length(min = 2))]
    pub country: String,
}

#[derive(Debug, Serialize, Deserialize, Validate, ToSchema)]
pub struct UpdatePurchaseOrderRequest {
    pub expected_delivery_date: Option<String>,
    pub shipping_address: Option<String>,
    pub notes: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Validate, ToSchema)]
pub struct ApprovePurchaseOrderRequest {
    pub approver_id: Uuid,
    pub notes: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Validate, ToSchema)]
pub struct CancelPurchaseOrderRequest {
    pub reason: String,
}

#[derive(Debug, Serialize, Deserialize, Validate, ToSchema)]
pub struct ReceivePurchaseOrderRequest {
    pub received_by: Uuid,
    pub notes: Option<String>,
    pub items_received: Vec<ItemReceivedRequest>,
}

#[derive(Debug, Serialize, Deserialize, Validate, ToSchema)]
pub struct ItemReceivedRequest {
    pub line_item_id: Uuid,

    pub quantity_received: i32,
    pub condition: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Validate, ToSchema)]
pub struct SubmitPurchaseOrderRequest {
    #[validate(length(max = 1000))]
    pub notes: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Validate, ToSchema)]
pub struct RejectPurchaseOrderRequest {
    pub rejector_id: Uuid,
    #[validate(length(min = 1, max = 500, message = "Rejection reason is required"))]
    pub reason: String,
    #[validate(length(max = 1000))]
    pub notes: Option<String>,
}

// Re-export DateRangeParams from common module
pub use crate::common::DateRangeParams;

// Handler functions

/// Create a new purchase order
#[utoipa::path(
    post,
    path = "/api/v1/purchase-orders",
    request_body = CreatePurchaseOrderRequest,
    responses(
        (status = 201, description = "Purchase order created", body = crate::ApiResponse<serde_json::Value>),
        (status = 400, description = "Invalid request", body = crate::errors::ErrorResponse)
    ),
    tag = "purchase-orders"
)]
pub async fn create_purchase_order(
    State(state): State<AppState>,
    _user: AuthenticatedUser,
    Json(payload): Json<CreatePurchaseOrderRequest>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    validate_input(&payload)?;

    let CreatePurchaseOrderRequest {
        supplier_id,
        expected_delivery_date,
        shipping_address,
        items,
        payment_terms,
        currency,
        notes,
    } = payload;

    let expected_delivery = NaiveDate::parse_from_str(&expected_delivery_date, "%Y-%m-%d")
        .map_err(|e| ApiError::ValidationError(format!("Invalid date format: {}", e)))?
        .and_hms_opt(0, 0, 0)
        .ok_or_else(|| ApiError::ValidationError("Invalid date format".to_string()))?;
    let expected_delivery: DateTime<Utc> =
        DateTime::<Utc>::from_naive_utc_and_offset(expected_delivery, Utc);

    let shipping_address = CommandShippingAddress {
        street: shipping_address.street,
        city: shipping_address.city,
        state: shipping_address.state,
        postal_code: shipping_address.postal_code,
        country: shipping_address.country,
    };

    let items = items
        .into_iter()
        .map(|item| CommandPurchaseOrderItemRequest {
            product_id: item.product_id,
            quantity: item.quantity,
            unit_price: item.unit_price,
            tax_rate: item.tax_rate,
            currency: item.currency,
            description: item.description,
        })
        .collect();

    let currency = currency.unwrap_or_else(|| "USD".to_string());

    let command = CreatePurchaseOrderCommand {
        supplier_id,
        items,
        expected_delivery_date: expected_delivery,
        shipping_address,
        payment_terms,
        currency,
        notes,
    };

    let po_id = state
        .services
        .procurement
        .create_purchase_order(command)
        .await
        .map_err(map_service_error)?;

    info!("Purchase order created: {}", po_id);

    Ok(created_response(serde_json::json!({
        "id": po_id,
        "message": "Purchase order created successfully"
    })))
}

/// Get a purchase order by ID
#[utoipa::path(
    get,
    path = "/api/v1/purchase-orders/{id}",
    params(
        ("id" = Uuid, Path, description = "Purchase order ID")
    ),
    responses(
        (status = 200, description = "Purchase order fetched", body = crate::ApiResponse<serde_json::Value>),
        (status = 404, description = "Purchase order not found", body = crate::errors::ErrorResponse)
    ),
    tag = "purchase-orders"
)]
pub async fn get_purchase_order(
    State(state): State<AppState>,
    Path(po_id): Path<Uuid>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    let po = state
        .services
        .procurement
        .get_purchase_order(&po_id)
        .await
        .map_err(map_service_error)?
        .ok_or_else(|| ApiError::NotFound(format!("Purchase order with ID {} not found", po_id)))?;

    Ok(success_response(po))
}

/// Update a purchase order
#[utoipa::path(
    put,
    path = "/api/v1/purchase-orders/{id}",
    request_body = UpdatePurchaseOrderRequest,
    params(
        ("id" = Uuid, Path, description = "Purchase order ID")
    ),
    responses(
        (status = 200, description = "Purchase order updated", body = crate::ApiResponse<serde_json::Value>),
        (status = 400, description = "Invalid request", body = crate::errors::ErrorResponse),
        (status = 404, description = "Purchase order not found", body = crate::errors::ErrorResponse)
    ),
    tag = "purchase-orders"
)]
pub async fn update_purchase_order(
    State(state): State<AppState>,
    Path(po_id): Path<Uuid>,
    Json(payload): Json<UpdatePurchaseOrderRequest>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    validate_input(&payload)?;

    // Parse the expected delivery date if provided
    let expected_delivery_date = if let Some(date_str) = &payload.expected_delivery_date {
        let date = NaiveDate::parse_from_str(date_str, "%Y-%m-%d")
            .map_err(|e| ApiError::ValidationError(format!("Invalid date format: {}", e)))?;
        Some(
            date.and_hms_opt(0, 0, 0)
                .ok_or_else(|| ApiError::ValidationError("Invalid datetime".to_string()))?,
        )
    } else {
        None
    };

    let command = UpdatePurchaseOrderCommand {
        id: po_id,
        expected_delivery_date,
        shipping_address: payload.shipping_address,
        notes: payload.notes,
        status: payload.status,
    };

    state
        .services
        .procurement
        .update_purchase_order(command)
        .await
        .map_err(map_service_error)?;

    info!("Purchase order updated: {}", po_id);

    Ok(success_response(serde_json::json!({
        "message": "Purchase order updated successfully"
    })))
}

/// Approve a purchase order
#[utoipa::path(
    post,
    path = "/api/v1/purchase-orders/{id}/approve",
    request_body = ApprovePurchaseOrderRequest,
    params(
        ("id" = Uuid, Path, description = "Purchase order ID")
    ),
    responses(
        (status = 200, description = "Purchase order approved", body = crate::ApiResponse<serde_json::Value>),
        (status = 400, description = "Invalid request", body = crate::errors::ErrorResponse),
        (status = 404, description = "Purchase order not found", body = crate::errors::ErrorResponse)
    ),
    tag = "purchase-orders"
)]
pub async fn approve_purchase_order(
    State(state): State<AppState>,
    Path(po_id): Path<Uuid>,
    Json(payload): Json<ApprovePurchaseOrderRequest>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    validate_input(&payload)?;

    let command = ApprovePurchaseOrderCommand {
        id: po_id,
        approver_id: payload.approver_id,
        notes: payload.notes,
    };

    state
        .services
        .procurement
        .approve_purchase_order(command)
        .await
        .map_err(map_service_error)?;

    info!("Purchase order approved: {}", po_id);

    Ok(success_response(serde_json::json!({
        "message": "Purchase order approved successfully"
    })))
}

/// Cancel a purchase order
#[utoipa::path(
    post,
    path = "/api/v1/purchase-orders/{id}/cancel",
    request_body = CancelPurchaseOrderRequest,
    params(
        ("id" = Uuid, Path, description = "Purchase order ID")
    ),
    responses(
        (status = 200, description = "Purchase order cancelled", body = crate::ApiResponse<serde_json::Value>),
        (status = 400, description = "Invalid request", body = crate::errors::ErrorResponse),
        (status = 404, description = "Purchase order not found", body = crate::errors::ErrorResponse)
    ),
    tag = "purchase-orders"
)]
pub async fn cancel_purchase_order(
    State(state): State<AppState>,
    Path(po_id): Path<Uuid>,
    Json(payload): Json<CancelPurchaseOrderRequest>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    validate_input(&payload)?;

    let command = CancelPurchaseOrderCommand {
        id: po_id,
        reason: payload.reason,
    };

    state
        .services
        .procurement
        .cancel_purchase_order(command)
        .await
        .map_err(map_service_error)?;

    info!("Purchase order cancelled: {}", po_id);

    Ok(success_response(serde_json::json!({
        "message": "Purchase order cancelled successfully"
    })))
}

/// Mark a purchase order as received
#[utoipa::path(
    post,
    path = "/api/v1/purchase-orders/{id}/receive",
    request_body = ReceivePurchaseOrderRequest,
    params(
        ("id" = Uuid, Path, description = "Purchase order ID")
    ),
    responses(
        (status = 200, description = "Purchase order received", body = crate::ApiResponse<serde_json::Value>),
        (status = 400, description = "Invalid request", body = crate::errors::ErrorResponse),
        (status = 404, description = "Purchase order not found", body = crate::errors::ErrorResponse)
    ),
    tag = "purchase-orders"
)]
pub async fn receive_purchase_order(
    State(state): State<AppState>,
    Path(po_id): Path<Uuid>,
    Json(payload): Json<ReceivePurchaseOrderRequest>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    validate_input(&payload)?;

    // Map the received items
    let items_received = payload
        .items_received
        .into_iter()
        .map(|item| (item.line_item_id, item.quantity_received, item.condition))
        .collect();

    let command = ReceivePurchaseOrderCommand {
        id: po_id,
        received_by: payload.received_by,
        notes: payload.notes,
        items_received,
    };

    state
        .services
        .procurement
        .receive_purchase_order(command)
        .await
        .map_err(map_service_error)?;

    info!("Purchase order received: {}", po_id);

    Ok(success_response(serde_json::json!({
        "message": "Purchase order received successfully"
    })))
}

/// Get purchase orders for a supplier
#[utoipa::path(
    get,
    path = "/api/v1/purchase-orders/supplier/{supplier_id}",
    params(
        ("supplier_id" = Uuid, Path, description = "Supplier ID")
    ),
    responses(
        (status = 200, description = "Purchase orders by supplier", body = crate::ApiResponse<serde_json::Value>)
    ),
    tag = "purchase-orders"
)]
pub async fn get_purchase_orders_by_supplier(
    State(state): State<AppState>,
    Path(supplier_id): Path<Uuid>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    let pos = state
        .services
        .procurement
        .get_purchase_orders_by_supplier(&supplier_id)
        .await
        .map_err(map_service_error)?;

    Ok(success_response(pos))
}

/// Get purchase orders by status
#[utoipa::path(
    get,
    path = "/api/v1/purchase-orders/status/{status}",
    params(
        ("status" = String, Path, description = "Purchase order status")
    ),
    responses(
        (status = 200, description = "Purchase orders by status", body = crate::ApiResponse<serde_json::Value>)
    ),
    tag = "purchase-orders"
)]
pub async fn get_purchase_orders_by_status(
    State(state): State<AppState>,
    Path(status): Path<String>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    let pos = state
        .services
        .procurement
        .get_purchase_orders_by_status(&status)
        .await
        .map_err(map_service_error)?;

    Ok(success_response(pos))
}

/// Get purchase orders by delivery date range
#[utoipa::path(
    get,
    path = "/api/v1/purchase-orders/delivery-date",
    params(DateRangeParams),
    responses(
        (status = 200, description = "Purchase orders by delivery date", body = crate::ApiResponse<serde_json::Value>),
        (status = 400, description = "Invalid date range", body = crate::errors::ErrorResponse)
    ),
    tag = "purchase-orders"
)]
pub async fn get_purchase_orders_by_delivery_date(
    State(state): State<AppState>,
    Query(params): Query<DateRangeParams>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    validate_input(&params)?;

    let (start_date, end_date) = params.to_datetime_range()?;

    let pos = state
        .services
        .procurement
        .get_purchase_orders_by_delivery_date(start_date, end_date)
        .await
        .map_err(map_service_error)?;

    Ok(success_response(pos))
}

/// Get total purchase value for a date range
#[utoipa::path(
    get,
    path = "/api/v1/purchase-orders/value",
    params(DateRangeParams),
    responses(
        (status = 200, description = "Total purchase value", body = crate::ApiResponse<serde_json::Value>),
        (status = 400, description = "Invalid date range", body = crate::errors::ErrorResponse)
    ),
    tag = "purchase-orders"
)]
pub async fn get_total_purchase_value(
    State(state): State<AppState>,
    Query(params): Query<DateRangeParams>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    validate_input(&params)?;

    let (start_date, end_date) = params.to_datetime_range()?;

    let total_value = state
        .services
        .procurement
        .get_total_purchase_value(start_date, end_date)
        .await
        .map_err(map_service_error)?;

    Ok(success_response(serde_json::json!({
        "total_value": total_value,
        "period": format!("{} to {}", params.start_date, params.end_date)
    })))
}

/// Submit a purchase order for approval
#[utoipa::path(
    post,
    path = "/api/v1/purchase-orders/{id}/submit",
    request_body = SubmitPurchaseOrderRequest,
    params(
        ("id" = Uuid, Path, description = "Purchase order ID")
    ),
    responses(
        (status = 200, description = "Purchase order submitted for approval", body = crate::ApiResponse<serde_json::Value>),
        (status = 400, description = "Invalid request or status", body = crate::errors::ErrorResponse),
        (status = 404, description = "Purchase order not found", body = crate::errors::ErrorResponse)
    ),
    tag = "purchase-orders"
)]
pub async fn submit_purchase_order(
    State(state): State<AppState>,
    _user: AuthenticatedUser,
    Path(id): Path<Uuid>,
    Json(payload): Json<SubmitPurchaseOrderRequest>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    validate_input(&payload)?;

    let command = SubmitPurchaseOrderCommand {
        id,
        notes: payload.notes,
    };

    let result = state
        .services
        .procurement
        .submit_purchase_order(command)
        .await
        .map_err(map_service_error)?;

    info!("Purchase order submitted: {} (status: {})", result.id, result.status);

    Ok(success_response(serde_json::json!({
        "id": result.id,
        "status": result.status,
        "submitted_at": result.submitted_at,
        "message": "Purchase order submitted for approval"
    })))
}

/// Reject a purchase order
#[utoipa::path(
    post,
    path = "/api/v1/purchase-orders/{id}/reject",
    request_body = RejectPurchaseOrderRequest,
    params(
        ("id" = Uuid, Path, description = "Purchase order ID")
    ),
    responses(
        (status = 200, description = "Purchase order rejected", body = crate::ApiResponse<serde_json::Value>),
        (status = 400, description = "Invalid request or status", body = crate::errors::ErrorResponse),
        (status = 404, description = "Purchase order not found", body = crate::errors::ErrorResponse)
    ),
    tag = "purchase-orders"
)]
pub async fn reject_purchase_order(
    State(state): State<AppState>,
    _user: AuthenticatedUser,
    Path(id): Path<Uuid>,
    Json(payload): Json<RejectPurchaseOrderRequest>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    validate_input(&payload)?;

    let command = RejectPurchaseOrderCommand {
        id,
        rejector_id: payload.rejector_id,
        reason: payload.reason.clone(),
        notes: payload.notes,
    };

    let result = state
        .services
        .procurement
        .reject_purchase_order(command)
        .await
        .map_err(map_service_error)?;

    info!("Purchase order rejected: {} (reason: {})", result.id, result.rejection_reason);

    Ok(success_response(serde_json::json!({
        "id": result.id,
        "status": result.status,
        "rejected_at": result.rejected_at,
        "rejection_reason": result.rejection_reason,
        "message": "Purchase order rejected"
    })))
}

/// Creates the router for purchase order endpoints
pub fn purchase_order_routes() -> Router<AppState> {
    Router::new()
        .route("/", post(create_purchase_order))
        .route("/{id}", get(get_purchase_order))
        .route("/{id}", put(update_purchase_order))
        .route("/{id}/submit", post(submit_purchase_order))
        .route("/{id}/approve", post(approve_purchase_order))
        .route("/{id}/reject", post(reject_purchase_order))
        .route("/{id}/cancel", post(cancel_purchase_order))
        .route("/{id}/receive", post(receive_purchase_order))
        .route(
            "/supplier/:supplier_id",
            get(get_purchase_orders_by_supplier),
        )
        .route("/status/:status", get(get_purchase_orders_by_status))
        .route("/delivery-date", get(get_purchase_orders_by_delivery_date))
        .route("/value", get(get_total_purchase_value))
}
