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
    services::returns::ReturnService,
    commands::returns::{
        create_return_command::CreateReturnCommand,
        approve_return_command::ApproveReturnCommand,
        reject_return_command::RejectReturnCommand,
        cancel_return_command::CancelReturnCommand,
        complete_return_command::CompleteReturnCommand,
        refund_return_command::RefundReturnCommand,
        restock_returned_items_command::RestockReturnedItemsCommand,
    },
    AppState,
};
use serde::{Serialize, Deserialize};
use validator::Validate;
use tracing::info;
use super::common::{validate_input, map_service_error, success_response, created_response, no_content_response, PaginationParams};

/// Creates the router for return endpoints
pub fn returns_routes() -> Router {
    Router::new()
        .route("/", get(list_returns))
        .route("/:id", get(get_return))
        .route("/", post(create_return))
        .route("/:id/approve", post(approve_return))
        .route("/:id/reject", post(reject_return))
        .route("/:id/cancel", post(cancel_return))
        .route("/:id/complete", post(complete_return))
        .route("/:id/refund", post(refund_return))
        .route("/:id/restock", post(restock_returned_items))
}

/// List returns with optional filtering and pagination
async fn list_returns(
    State(state): State<Arc<AppState>>,
    AuthenticatedUser(_user): AuthenticatedUser,
    Query(params): Query<PaginationParams>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    let (returns, total) = state.services.returns
        .list_returns(params.page.unwrap_or(1), params.limit.unwrap_or(20))
        .await
        .map_err(map_service_error)?;
    
    success_response(serde_json::json!({
        "returns": returns,
        "total": total,
        "page": params.page.unwrap_or(1),
        "limit": params.limit.unwrap_or(20)
    }))
}

// Request and response DTOs

#[derive(Debug, Deserialize, Validate)]
pub struct CreateReturnRequest {
    pub order_id: Uuid,
    #[validate(length(min = 1, message = "Reason cannot be empty"))]
    pub reason: String,
}

#[derive(Debug, Deserialize, Validate)]
pub struct ApproveReturnRequest {
    pub approved_by: Uuid,
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct RejectReturnRequest {
    pub rejected_by: Uuid,
    #[validate(length(min = 1, message = "Reason cannot be empty"))]
    pub reason: String,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CancelReturnRequest {
    #[validate(length(min = 1, message = "Reason cannot be empty"))]
    pub reason: String,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CompleteReturnRequest {
    pub completed_by: Uuid,
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct RefundReturnRequest {
    pub amount: f64,
    pub refund_method: String,
    pub refund_reference: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct RestockReturnedItemsRequest {
    pub items: Vec<RestockItemRequest>,
    pub location_id: Uuid,
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct RestockItemRequest {
    pub return_item_id: Uuid,
    #[validate(range(min = 1, message = "Quantity must be at least 1"))]
    pub quantity: i32,
    pub condition: String,
}

// Handler functions

/// Create a new return
async fn create_return(
    State(state): State<Arc<AppState>>,
    AuthenticatedUser(user): AuthenticatedUser,
    Json(payload): Json<CreateReturnRequest>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    validate_input(&payload)?;
    
    let command = crate::commands::returns::create_return_command::InitiateReturnCommand {
        order_id: payload.order_id,
        reason: payload.reason,
    };
    
    let return_id = state.services.returns
        .create_return(command)
        .await
        .map_err(map_service_error)?;
    
    info!("Return created: {}", return_id);
    
    created_response(serde_json::json!({
        "id": return_id,
        "message": "Return created successfully"
    }))
}

/// Get a return by ID
async fn get_return(
    State(state): State<Arc<AppState>>,
    Path(return_id): Path<Uuid>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    let ret = state.services.returns
        .get_return(&return_id)
        .await
        .map_err(map_service_error)?
        .ok_or_else(|| ApiError::NotFound(format!("Return with ID {} not found", return_id)))?;
    
    success_response(ret)
}

/// Approve a return
async fn approve_return(
    State(state): State<Arc<AppState>>,
    Path(return_id): Path<Uuid>,
    Json(payload): Json<ApproveReturnRequest>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    validate_input(&payload)?;
    
    let command = ApproveReturnCommand {
        id: return_id,
        approved_by: payload.approved_by,
        notes: payload.notes,
    };
    
    state.services.returns
        .approve_return(command)
        .await
        .map_err(map_service_error)?;
    
    info!("Return approved: {}", return_id);
    
    success_response(serde_json::json!({
        "message": "Return approved successfully"
    }))
}

/// Reject a return
async fn reject_return(
    State(state): State<Arc<AppState>>,
    Path(return_id): Path<Uuid>,
    Json(payload): Json<RejectReturnRequest>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    validate_input(&payload)?;
    
    let command = RejectReturnCommand {
        id: return_id,
        rejected_by: payload.rejected_by,
        reason: payload.reason,
    };
    
    state.services.returns
        .reject_return(command)
        .await
        .map_err(map_service_error)?;
    
    info!("Return rejected: {}", return_id);
    
    success_response(serde_json::json!({
        "message": "Return rejected successfully"
    }))
}

/// Cancel a return
async fn cancel_return(
    State(state): State<Arc<AppState>>,
    Path(return_id): Path<Uuid>,
    Json(payload): Json<CancelReturnRequest>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    validate_input(&payload)?;
    
    let command = CancelReturnCommand {
        id: return_id,
        reason: payload.reason,
    };
    
    state.services.returns
        .cancel_return(command)
        .await
        .map_err(map_service_error)?;
    
    info!("Return cancelled: {}", return_id);
    
    success_response(serde_json::json!({
        "message": "Return cancelled successfully"
    }))
}

/// Complete a return
async fn complete_return(
    State(state): State<Arc<AppState>>,
    Path(return_id): Path<Uuid>,
    Json(payload): Json<CompleteReturnRequest>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    validate_input(&payload)?;
    
    let command = CompleteReturnCommand {
        id: return_id,
        completed_by: payload.completed_by,
        notes: payload.notes,
    };
    
    state.services.returns
        .complete_return(command)
        .await
        .map_err(map_service_error)?;
    
    info!("Return completed: {}", return_id);
    
    success_response(serde_json::json!({
        "message": "Return completed successfully"
    }))
}

/// Process a refund for a return
async fn refund_return(
    State(state): State<Arc<AppState>>,
    Path(return_id): Path<Uuid>,
    Json(payload): Json<RefundReturnRequest>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    validate_input(&payload)?;
    
    let command = RefundReturnCommand {
        id: return_id,
        amount: payload.amount,
        refund_method: payload.refund_method,
        refund_reference: payload.refund_reference,
        notes: payload.notes,
    };
    
    state.services.returns
        .refund_return(command)
        .await
        .map_err(map_service_error)?;
    
    info!("Return refunded: {}", return_id);
    
    success_response(serde_json::json!({
        "message": "Return refunded successfully"
    }))
}

/// Restock items from a return
async fn restock_returned_items(
    State(state): State<Arc<AppState>>,
    Path(return_id): Path<Uuid>,
    Json(payload): Json<RestockReturnedItemsRequest>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    validate_input(&payload)?;
    
    // Map the restock items
    let items = payload.items.into_iter()
        .map(|item| (item.return_item_id, item.quantity, item.condition))
        .collect();
    
    let command = RestockReturnedItemsCommand {
        return_id,
        items,
        location_id: payload.location_id,
        notes: payload.notes,
    };
    
    state.services.returns
        .restock_returned_items(command)
        .await
        .map_err(map_service_error)?;
    
    info!("Return items restocked: {}", return_id);
    
    success_response(serde_json::json!({
        "message": "Return items restocked successfully"
    }))
}

