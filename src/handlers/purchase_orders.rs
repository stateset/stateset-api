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
        update_purchase_order_command::UpdatePurchaseOrderCommand,
    },
    errors::{ApiError, ServiceError},
    handlers::AppState,
};
use axum::{
    extract::{Json, Path, Query, State},
    routing::{delete, get, post, put},
    Router,
};
use chrono::{DateTime, NaiveDate, NaiveDateTime, Utc};
use serde::{Deserialize, Serialize};
use tracing::{error, info};
use uuid::Uuid;
use validator::Validate;

// Request and response DTOs

#[derive(Debug, Serialize, Deserialize, Validate)]
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

#[derive(Debug, Serialize, Deserialize, Validate)]
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

#[derive(Debug, Serialize, Deserialize, Validate)]
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

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct UpdatePurchaseOrderRequest {
    pub expected_delivery_date: Option<String>,
    pub shipping_address: Option<String>,
    pub notes: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct ApprovePurchaseOrderRequest {
    pub approver_id: Uuid,
    pub notes: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct CancelPurchaseOrderRequest {
    pub reason: String,
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct ReceivePurchaseOrderRequest {
    pub received_by: Uuid,
    pub notes: Option<String>,
    pub items_received: Vec<ItemReceivedRequest>,
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct ItemReceivedRequest {
    pub line_item_id: Uuid,
    
    pub quantity_received: i32,
    pub condition: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct DateRangeParams {
    
    pub start_date: String,
    
    pub end_date: String,
}

impl DateRangeParams {
    /// Converts string dates to NaiveDateTime
    pub fn to_datetime_range(&self) -> Result<(NaiveDateTime, NaiveDateTime), ApiError> {
        let start_date = NaiveDate::parse_from_str(&self.start_date, "%Y-%m-%d")
            .map_err(|e| ApiError::ValidationError(format!("Invalid start date format: {}", e)))?;

        let end_date = NaiveDate::parse_from_str(&self.end_date, "%Y-%m-%d")
            .map_err(|e| ApiError::ValidationError(format!("Invalid end date format: {}", e)))?;

        Ok((
            start_date.and_hms_opt(0, 0, 0).unwrap(),
            end_date.and_hms_opt(23, 59, 59).unwrap(),
        ))
    }
}

// Handler functions

/// Create a new purchase order
async fn create_purchase_order(
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
async fn get_purchase_order(
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
async fn update_purchase_order(
    State(state): State<AppState>,
    Path(po_id): Path<Uuid>,
    Json(payload): Json<UpdatePurchaseOrderRequest>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    validate_input(&payload)?;

    // Parse the expected delivery date if provided
    let expected_delivery_date = if let Some(date_str) = &payload.expected_delivery_date {
        Some(
            NaiveDate::parse_from_str(date_str, "%Y-%m-%d")
                .map_err(|e| ApiError::ValidationError(format!("Invalid date format: {}", e)))?
                .and_hms_opt(0, 0, 0)
                .unwrap(),
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
async fn approve_purchase_order(
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
async fn cancel_purchase_order(
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
async fn receive_purchase_order(
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
async fn get_purchase_orders_by_supplier(
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
async fn get_purchase_orders_by_status(
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
async fn get_purchase_orders_by_delivery_date(
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
async fn get_total_purchase_value(
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

/// Creates the router for purchase order endpoints
pub fn purchase_order_routes() -> Router<AppState> {
    Router::new()
        .route("/", post(create_purchase_order))
        .route("/{id}", get(get_purchase_order))
        .route("/{id}", put(update_purchase_order))
        .route("/{id}/approve", post(approve_purchase_order))
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
