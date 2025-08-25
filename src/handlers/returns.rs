use crate::errors::ServiceError;
use axum::{
    extract::{Json, Path, Query, State},
    response::IntoResponse,
    routing::{delete, get, post, put},
    Router,
    http::StatusCode,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::{ToSchema, IntoParams};
use serde_json::json;
use uuid::Uuid;
use std::sync::Arc;

// Generic trait for returns handler state
pub trait ReturnsAppState: Clone + Send + Sync + 'static {}
impl<T> ReturnsAppState for T where T: Clone + Send + Sync + 'static {}

#[derive(Debug, Serialize, Deserialize)]
#[derive(ToSchema)]
pub struct Return {
    pub id: String,
    pub order_id: String,
    pub customer_id: String,
    pub status: String,
    pub reason: String,
    pub description: Option<String>,
    pub return_type: String, // "exchange", "refund", "store_credit"
    pub items: Vec<ReturnItem>,
    pub total_refund_amount: f64,
    pub inspection_notes: Option<String>,
    pub processed_by: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub approved_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ReturnItem {
    pub id: String,
    pub order_item_id: String,
    pub product_id: String,
    pub quantity: i32,
    pub unit_price: f64,
    pub total_refund: f64,
    pub condition: String, // "new", "used", "damaged", "defective"
    pub restockable: bool,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateReturnRequest {
    pub order_id: String,
    pub reason: String,
    pub description: Option<String>,
    pub return_type: String,
    pub items: Vec<CreateReturnItemRequest>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateReturnItemRequest {
    pub order_item_id: String,
    pub quantity: i32,
    pub reason: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateReturnRequest {
    pub status: Option<String>,
    pub inspection_notes: Option<String>,
    pub reason: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ProcessReturnRequest {
    pub action: String, // "approve", "reject", "partial_approve"
    pub inspection_notes: String,
    pub items: Vec<ProcessReturnItemRequest>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ProcessReturnItemRequest {
    pub return_item_id: String,
    pub approved_quantity: i32,
    pub condition: String,
    pub restockable: bool,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct RestockReturnRequest {
    pub return_id: String,
    pub location_id: String,
    pub items: Vec<RestockItemRequest>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct RestockItemRequest {
    pub return_item_id: String,
    pub quantity: i32,
    pub condition: String,
}

#[derive(Debug, Deserialize, ToSchema, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ReturnFilters {
    pub status: Option<String>,
    pub customer_id: Option<String>,
    pub order_id: Option<String>,
    pub return_type: Option<String>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateReturnStatusBody {
    pub status: String,
}

/// Create the returns router
pub fn returns_router<S>() -> Router<S> 
where 
    S: ReturnsAppState,
{
    Router::new()
        .route("/", get(list_returns::<S>).post(create_return::<S>))
        .route("/{id}", get(get_return::<S>).put(update_return::<S>).delete(delete_return::<S>))
        .route("/{id}/process", post(process_return::<S>))
        .route("/{id}/approve", post(approve_return::<S>))
        .route("/{id}/reject", post(reject_return::<S>))
        .route("/{id}/restock", post(restock_return::<S>))
        .route("/{id}/refund", post(issue_refund::<S>))
}

/// List returns with optional filtering
#[utoipa::path(
    get,
    path = "/api/v1/returns",
    params(ReturnFilters),
    responses(
        (status = 200, description = "List returns",
            headers(
                ("X-Request-Id" = String, description = "Unique request id"),
                ("X-RateLimit-Limit" = String, description = "Requests allowed in current window"),
                ("X-RateLimit-Remaining" = String, description = "Remaining requests in window"),
                ("X-RateLimit-Reset" = String, description = "Seconds until reset"),
            )
        ),
        (status = 401, description = "Unauthorized", body = crate::errors::ErrorResponse),
        (status = 403, description = "Forbidden", body = crate::errors::ErrorResponse),
        (status = 429, description = "Rate limit exceeded", body = crate::errors::ErrorResponse),
        (status = 500, description = "Internal server error", body = crate::errors::ErrorResponse)
    ),
    tag = "returns"
)]
pub async fn list_returns<S>(
    State(_state): State<S>,
    Query(filters): Query<ReturnFilters>,
) -> Result<impl IntoResponse, ServiceError> 
where 
    S: ReturnsAppState,
{
    // Mock data for now - replace with actual database queries
    let mut returns = vec![
        Return {
            id: "ret_001".to_string(),
            order_id: "order_001".to_string(),
            customer_id: "cust_123".to_string(),
            status: "pending".to_string(),
            reason: "defective".to_string(),
            description: Some("Item arrived damaged".to_string()),
            return_type: "refund".to_string(),
            items: vec![
                ReturnItem {
                    id: "ret_item_001".to_string(),
                    order_item_id: "order_item_001".to_string(),
                    product_id: "prod_abc".to_string(),
                    quantity: 1,
                    unit_price: 99.99,
                    total_refund: 99.99,
                    condition: "damaged".to_string(),
                    restockable: false,
                }
            ],
            total_refund_amount: 99.99,
            inspection_notes: None,
            processed_by: None,
            created_at: Utc::now() - chrono::Duration::hours(2),
            updated_at: Utc::now(),
            approved_at: None,
            completed_at: None,
        },
        Return {
            id: "ret_002".to_string(),
            order_id: "order_002".to_string(),
            customer_id: "cust_456".to_string(),
            status: "approved".to_string(),
            reason: "size_issue".to_string(),
            description: Some("Wrong size ordered".to_string()),
            return_type: "exchange".to_string(),
            items: vec![
                ReturnItem {
                    id: "ret_item_002".to_string(),
                    order_item_id: "order_item_002".to_string(),
                    product_id: "prod_def".to_string(),
                    quantity: 1,
                    unit_price: 149.99,
                    total_refund: 0.0,
                    condition: "new".to_string(),
                    restockable: true,
                }
            ],
            total_refund_amount: 0.0,
            inspection_notes: Some("Item in excellent condition".to_string()),
            processed_by: Some("staff_001".to_string()),
            created_at: Utc::now() - chrono::Duration::days(1),
            updated_at: Utc::now() - chrono::Duration::hours(1),
            approved_at: Some(Utc::now() - chrono::Duration::hours(1)),
            completed_at: None,
        },
    ];

    // Apply filters
    if let Some(status) = &filters.status {
        returns.retain(|r| &r.status == status);
    }
    if let Some(customer_id) = &filters.customer_id {
        returns.retain(|r| &r.customer_id == customer_id);
    }
    if let Some(order_id) = &filters.order_id {
        returns.retain(|r| &r.order_id == order_id);
    }
    if let Some(return_type) = &filters.return_type {
        returns.retain(|r| &r.return_type == return_type);
    }

    let response = json!({
        "returns": returns,
        "total": returns.len(),
        "limit": filters.limit.unwrap_or(50),
        "offset": filters.offset.unwrap_or(0)
    });

    Ok((StatusCode::OK, Json(response)))
}

/// Create a new return
#[utoipa::path(
    post,
    path = "/api/v1/returns",
    request_body = CreateReturnRequest,
    responses(
        (status = 201, description = "Return created", body = Return,
            headers(("X-Request-Id" = String, description = "Unique request id"))
        ),
        (status = 400, description = "Invalid request", body = crate::errors::ErrorResponse),
        (status = 401, description = "Unauthorized", body = crate::errors::ErrorResponse),
        (status = 403, description = "Forbidden", body = crate::errors::ErrorResponse),
        (status = 422, description = "Validation error", body = crate::errors::ErrorResponse),
        (status = 429, description = "Rate limit exceeded", body = crate::errors::ErrorResponse)
    ),
    tag = "returns"
)]
pub async fn create_return<S>(
    State(_state): State<S>,
    Json(payload): Json<CreateReturnRequest>,
) -> Result<impl IntoResponse, ServiceError> 
where 
    S: ReturnsAppState,
{
    let return_id = Uuid::new_v4().to_string();
    let now = Utc::now();
    
    // Calculate total refund amount from items
    let total_refund_amount = match payload.return_type.as_str() {
        "refund" => 99.99, // Mock calculation
        _ => 0.0,
    };
    
    let return_order = Return {
        id: return_id.clone(),
        order_id: payload.order_id,
        customer_id: "cust_123".to_string(), // Mock - get from order
        status: "pending".to_string(),
        reason: payload.reason,
        description: payload.description,
        return_type: payload.return_type,
        items: payload.items.into_iter().enumerate().map(|(i, item)| ReturnItem {
            id: format!("{}_item_{}", return_id, i),
            order_item_id: item.order_item_id,
            product_id: "prod_abc".to_string(), // Mock - get from order item
            quantity: item.quantity,
            unit_price: 99.99, // Mock - get from order item
            total_refund: 99.99 * item.quantity as f64,
            condition: "unknown".to_string(),
            restockable: true,
        }).collect(),
        total_refund_amount,
        inspection_notes: None,
        processed_by: None,
        created_at: now,
        updated_at: now,
        approved_at: None,
        completed_at: None,
    };

    Ok((StatusCode::CREATED, Json(return_order)))
}

/// Get a specific return by ID
#[utoipa::path(
    get,
    path = "/api/v1/returns/{id}",
    params(("id" = String, Path, description = "Return ID")),
    responses((status = 200, description = "Return details", body = Return)),
    tag = "returns"
)]
pub async fn get_return<S>(
    State(_state): State<S>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, ServiceError> 
where 
    S: ReturnsAppState,
{
    let return_order = Return {
        id: id.clone(),
        order_id: "order_001".to_string(),
        customer_id: "cust_123".to_string(),
        status: "pending".to_string(),
        reason: "defective".to_string(),
        description: Some("Item arrived damaged".to_string()),
        return_type: "refund".to_string(),
        items: vec![
            ReturnItem {
                id: format!("{}_item_1", id),
                order_item_id: "order_item_001".to_string(),
                product_id: "prod_abc".to_string(),
                quantity: 1,
                unit_price: 99.99,
                total_refund: 99.99,
                condition: "damaged".to_string(),
                restockable: false,
            }
        ],
        total_refund_amount: 99.99,
        inspection_notes: None,
        processed_by: None,
        created_at: Utc::now() - chrono::Duration::hours(2),
        updated_at: Utc::now(),
        approved_at: None,
        completed_at: None,
    };

    Ok((StatusCode::OK, Json(return_order)))
}

/// Update a return
#[utoipa::path(
    put,
    path = "/api/v1/returns/{id}",
    params(("id" = String, Path, description = "Return ID")),
    request_body = UpdateReturnRequest,
    responses((status = 200, description = "Return updated", body = Return)),
    tag = "returns"
)]
pub async fn update_return<S>(
    State(_state): State<S>,
    Path(id): Path<String>,
    Json(payload): Json<UpdateReturnRequest>,
) -> Result<impl IntoResponse, ServiceError> 
where 
    S: ReturnsAppState,
{
    let return_order = Return {
        id: id.clone(),
        order_id: "order_001".to_string(),
        customer_id: "cust_123".to_string(),
        status: payload.status.unwrap_or_else(|| "pending".to_string()),
        reason: payload.reason.unwrap_or_else(|| "defective".to_string()),
        description: Some("Item arrived damaged".to_string()),
        return_type: "refund".to_string(),
        items: vec![],
        total_refund_amount: 99.99,
        inspection_notes: payload.inspection_notes,
        processed_by: Some("staff_001".to_string()),
        created_at: Utc::now() - chrono::Duration::hours(2),
        updated_at: Utc::now(),
        approved_at: None,
        completed_at: None,
    };

    Ok((StatusCode::OK, Json(return_order)))
}

/// Update return status
#[utoipa::path(
    put,
    path = "/api/v1/returns/{id}/status",
    params(("id" = String, Path, description = "Return ID")),
    request_body = UpdateReturnStatusBody,
    responses(
        (status = 200, description = "Status updated",
            headers(("X-Request-Id" = String, description = "Unique request id"))
        ),
        (status = 401, description = "Unauthorized", body = crate::errors::ErrorResponse),
        (status = 403, description = "Forbidden", body = crate::errors::ErrorResponse)
    ),
    tag = "returns"
)]
pub async fn update_return_status<S>(
    State(_state): State<S>,
    Path(id): Path<String>,
    Json(payload): Json<UpdateReturnStatusBody>,
) -> Result<impl IntoResponse, ServiceError> 
where 
    S: ReturnsAppState,
{
    let new_status = payload.status.as_str();
    
    let response = json!({
        "message": format!("Return {} status updated to {}", id, new_status),
        "return_id": id,
        "status": new_status,
        "updated_at": Utc::now()
    });

    Ok((StatusCode::OK, Json(response)))
}

/// Delete a return
#[utoipa::path(
    delete,
    path = "/api/v1/returns/{id}",
    params(("id" = String, Path, description = "Return ID")),
    responses((status = 200, description = "Return deleted")),
    tag = "returns"
)]
pub async fn delete_return<S>(
    State(_state): State<S>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, ServiceError> 
where 
    S: ReturnsAppState,
{
    let response = json!({
        "message": format!("Return {} has been deleted", id),
        "deleted_id": id
    });

    Ok((StatusCode::OK, Json(response)))
}

/// Process a return (inspect and make approval decision)
#[utoipa::path(
    post,
    path = "/api/v1/returns/{id}/process",
    params(("id" = String, Path, description = "Return ID")),
    request_body = ProcessReturnRequest,
    responses(
        (status = 200, description = "Return processed",
            headers(("X-Request-Id" = String, description = "Unique request id"))
        ),
        (status = 400, description = "Invalid request", body = crate::errors::ErrorResponse),
        (status = 401, description = "Unauthorized", body = crate::errors::ErrorResponse),
        (status = 403, description = "Forbidden", body = crate::errors::ErrorResponse)
    ),
    tag = "returns"
)]
pub async fn process_return<S>(
    State(_state): State<S>,
    Path(id): Path<String>,
    Json(payload): Json<ProcessReturnRequest>,
) -> Result<impl IntoResponse, ServiceError> 
where 
    S: ReturnsAppState,
{
    let status = match payload.action.as_str() {
        "approve" => "approved",
        "reject" => "rejected",
        "partial_approve" => "partially_approved",
        _ => "pending",
    };

    let response = json!({
        "message": format!("Return {} has been {}", id, payload.action),
        "return_id": id,
        "status": status,
        "inspection_notes": payload.inspection_notes,
        "processed_at": Utc::now()
    });

    Ok((StatusCode::OK, Json(response)))
}

/// Approve a return
#[utoipa::path(
    post,
    path = "/api/v1/returns/{id}/approve",
    params(("id" = String, Path, description = "Return ID")),
    responses(
        (status = 200, description = "Return approved",
            headers(("X-Request-Id" = String, description = "Unique request id"))
        ),
        (status = 401, description = "Unauthorized", body = crate::errors::ErrorResponse),
        (status = 403, description = "Forbidden", body = crate::errors::ErrorResponse)
    ),
    tag = "returns"
)]
pub async fn approve_return<S>(
    State(_state): State<S>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, ServiceError> 
where 
    S: ReturnsAppState,
{
    let response = json!({
        "message": format!("Return {} has been approved", id),
        "return_id": id,
        "status": "approved",
        "approved_at": Utc::now()
    });

    Ok((StatusCode::OK, Json(response)))
}

/// Reject a return
#[utoipa::path(
    post,
    path = "/api/v1/returns/{id}/reject",
    params(("id" = String, Path, description = "Return ID")),
    responses(
        (status = 200, description = "Return rejected",
            headers(("X-Request-Id" = String, description = "Unique request id"))
        ),
        (status = 401, description = "Unauthorized", body = crate::errors::ErrorResponse),
        (status = 403, description = "Forbidden", body = crate::errors::ErrorResponse)
    ),
    tag = "returns"
)]
pub async fn reject_return<S>(
    State(_state): State<S>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, ServiceError> 
where 
    S: ReturnsAppState,
{
    let response = json!({
        "message": format!("Return {} has been rejected", id),
        "return_id": id,
        "status": "rejected",
        "rejected_at": Utc::now()
    });

    Ok((StatusCode::OK, Json(response)))
}

/// Restock returned items
#[utoipa::path(
    post,
    path = "/api/v1/returns/{id}/restock",
    params(("id" = String, Path, description = "Return ID")),
    request_body = RestockReturnRequest,
    responses(
        (status = 200, description = "Items restocked",
            headers(("X-Request-Id" = String, description = "Unique request id"))
        ),
        (status = 400, description = "Invalid request", body = crate::errors::ErrorResponse),
        (status = 401, description = "Unauthorized", body = crate::errors::ErrorResponse),
        (status = 403, description = "Forbidden", body = crate::errors::ErrorResponse)
    ),
    tag = "returns"
)]
pub async fn restock_return<S>(
    State(_state): State<S>,
    Path(id): Path<String>,
    Json(payload): Json<RestockReturnRequest>,
) -> Result<impl IntoResponse, ServiceError> 
where 
    S: ReturnsAppState,
{
    let response = json!({
        "message": format!("Return {} items have been restocked", id),
        "return_id": id,
        "location_id": payload.location_id,
        "restocked_items": payload.items.len(),
        "restocked_at": Utc::now()
    });

    Ok((StatusCode::OK, Json(response)))
}

/// Issue refund for return
#[utoipa::path(
    post,
    path = "/api/v1/returns/{id}/refund",
    params(("id" = String, Path, description = "Return ID")),
    responses((status = 200, description = "Refund issued")),
    tag = "returns"
)]
pub async fn issue_refund<S>(
    State(_state): State<S>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, ServiceError> 
where 
    S: ReturnsAppState,
{
    let response = json!({
        "message": format!("Refund issued for return {}", id),
        "return_id": id,
        "refund_amount": 99.99,
        "refund_method": "original_payment",
        "refund_id": Uuid::new_v4().to_string(),
        "issued_at": Utc::now()
    });

    Ok((StatusCode::OK, Json(response)))
}
