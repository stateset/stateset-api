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
    services::work_orders::WorkOrderService,
    commands::workorders::{
        create_work_order_command::CreateWorkOrderCommand,
        update_work_order_command::UpdateWorkOrderCommand,
        cancel_work_order_command::CancelWorkOrderCommand,
        start_work_order_command::StartWorkOrderCommand,
        complete_work_order_command::CompleteWorkOrderCommand,
        assign_work_order_command::AssignWorkOrderCommand,
        unassign_work_order_command::UnassignWorkOrderCommand,
        schedule_work_order_command::ScheduleWorkOrderCommand,
    },
    AppState,
};
use serde::{Serialize, Deserialize};
use validator::Validate;
use tracing::info;
use chrono::{NaiveDateTime, NaiveDate};
use super::common::{validate_input, map_service_error, success_response, created_response, no_content_response, PaginationParams};

/// Creates the router for work order endpoints
pub fn work_orders_routes() -> Router {
    Router::new()
        .route("/", get(list_work_orders))
        .route("/", post(create_work_order))
        .route("/:id", get(get_work_order))
        .route("/:id", put(update_work_order))
        .route("/:id/cancel", post(cancel_work_order))
        .route("/:id/start", post(start_work_order))
        .route("/:id/complete", post(complete_work_order))
        .route("/:id/assign", post(assign_work_order))
        .route("/:id/unassign", post(unassign_work_order))
        .route("/:id/schedule", post(schedule_work_order))
        .route("/assignee/:user_id", get(get_work_orders_by_assignee))
        .route("/status/:status", get(get_work_orders_by_status))
        .route("/schedule", get(get_work_orders_by_schedule))
}

/// List all work orders with pagination
async fn list_work_orders(
    State(state): State<Arc<AppState>>,
    Query(pagination): Query<PaginationParams>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    let page = pagination.page;
    let per_page = pagination.per_page;
    
    if page == 0 {
        return Err(ApiError::BadRequest("Page number must be at least 1".to_string()));
    }
    
    if per_page == 0 || per_page > 100 {
        return Err(ApiError::BadRequest("Page size must be between 1 and 100".to_string()));
    }
    
    // TODO: Add count query for total items when pagination is improved
    let work_orders = state.services.work_orders
        .list_work_orders(page, per_page)
        .await
        .map_err(map_service_error)?;
    
    success_response(serde_json::json!({
        "data": work_orders,
        "pagination": {
            "page": page,
            "per_page": per_page,
            "total_pages": 1, // This should be calculated based on total count
            "total_items": work_orders.len() // This should be the actual total count
        }
    }))
}

/// Get work order by ID
async fn get_work_order(
    State(state): State<Arc<AppState>>,
    Path(work_order_id): Path<Uuid>
) -> Result<impl axum::response::IntoResponse, ApiError> {
    let work_order = state.services.work_orders
        .get_work_order(&work_order_id)
        .await
        .map_err(map_service_error)?
        .ok_or_else(|| ApiError::NotFound(format!("Work order with ID {} not found", work_order_id)))?;
    
    success_response(work_order)
}

/// Create a new work order
async fn create_work_order(
    State(state): State<Arc<AppState>>,
    AuthenticatedUser(user): AuthenticatedUser,
    Json(payload): Json<CreateWorkOrderRequest>
) -> Result<impl axum::response::IntoResponse, ApiError> {
    validate_input(&payload)?;
    
    // Convert scheduled dates if provided
    let scheduled_start = payload.scheduled_start_date.as_ref().map(|date_str| {
        NaiveDate::parse_from_str(date_str, "%Y-%m-%d")
            .map_err(|e| ApiError::BadRequest(format!("Invalid start date format: {}", e)))
            .map(|date| date.and_hms_opt(0, 0, 0).unwrap())
    }).transpose()?;
    
    let scheduled_end = payload.scheduled_end_date.as_ref().map(|date_str| {
        NaiveDate::parse_from_str(date_str, "%Y-%m-%d")
            .map_err(|e| ApiError::BadRequest(format!("Invalid end date format: {}", e)))
            .map(|date| date.and_hms_opt(23, 59, 59).unwrap())
    }).transpose()?;
    
    let command = CreateWorkOrderCommand {
        bom_id: payload.bom_id,
        quantity_planned: payload.quantity_planned,
        priority: payload.priority,
        notes: payload.notes,
        scheduled_start_date: scheduled_start,
        scheduled_end_date: scheduled_end,
    };
    
    let work_order_id = state.services.work_orders
        .create_work_order(command)
        .await
        .map_err(map_service_error)?;
    
    info!("Work order created: {}", work_order_id);
    
    created_response(serde_json::json!({
        "id": work_order_id,
        "message": "Work order created successfully"
    }))
}

// Request and response DTOs

#[derive(Debug, Deserialize, Validate)]
pub struct CreateWorkOrderRequest {
    pub bom_id: Uuid,
    #[validate(range(min = 1, message = "Quantity must be at least 1"))]
    pub quantity_planned: i32,
    pub priority: String,
    pub notes: String,
    pub scheduled_start_date: Option<String>,
    pub scheduled_end_date: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateWorkOrderRequest {
    pub quantity_planned: Option<i32>,
    pub priority: Option<String>,
    pub notes: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CancelWorkOrderRequest {
    pub reason: String,
}

#[derive(Debug, Deserialize, Validate)]
pub struct StartWorkOrderRequest {
    pub started_by: Uuid,
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CompleteWorkOrderRequest {
    pub completed_by: Uuid,
    #[validate(range(min = 0, message = "Quantity must be at least 0"))]
    pub quantity_completed: i32,
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct AssignWorkOrderRequest {
    pub assigned_to: Uuid,
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct ScheduleWorkOrderRequest {
    pub start_date: String,
    pub end_date: String,
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct DateRangeParams {
    #[validate]
    pub start_date: String,
    #[validate]
    pub end_date: String,
}

impl DateRangeParams {
    /// Converts string dates to NaiveDateTime
    pub fn to_datetime_range(&self) -> Result<(NaiveDateTime, NaiveDateTime), ApiError> {
        let start_date = NaiveDate::parse_from_str(&self.start_date, "%Y-%m-%d")
            .map_err(|e| ApiError::BadRequest(format\!("Invalid start date format: {}", e)))?;
        
        let end_date = NaiveDate::parse_from_str(&self.end_date, "%Y-%m-%d")
            .map_err(|e| ApiError::BadRequest(format\!("Invalid end date format: {}", e)))?;
        
        Ok((
            start_date.and_hms_opt(0, 0, 0).unwrap(),
            end_date.and_hms_opt(23, 59, 59).unwrap(),
        ))
    }
}

// Handler functions

/// Create a new work order
async fn create_work_order(
    State(state): State<Arc<AppState>>,
    AuthenticatedUser(user): AuthenticatedUser,
    Json(payload): Json<CreateWorkOrderRequest>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    validate_input(&payload)?;
    
    // Convert scheduled dates if provided
    let scheduled_start = payload.scheduled_start_date.as_ref().map(|date_str| {
        NaiveDate::parse_from_str(date_str, "%Y-%m-%d")
            .map_err(|e| ApiError::BadRequest(format\!("Invalid start date format: {}", e)))
            .map(|date| date.and_hms_opt(0, 0, 0).unwrap())
    }).transpose()?;
    
    let scheduled_end = payload.scheduled_end_date.as_ref().map(|date_str| {
        NaiveDate::parse_from_str(date_str, "%Y-%m-%d")
            .map_err(|e| ApiError::BadRequest(format\!("Invalid end date format: {}", e)))
            .map(|date| date.and_hms_opt(23, 59, 59).unwrap())
    }).transpose()?;
    
    let command = CreateWorkOrderCommand {
        bom_id: payload.bom_id,
        quantity_planned: payload.quantity_planned,
        priority: payload.priority,
        notes: payload.notes,
        scheduled_start_date: scheduled_start,
        scheduled_end_date: scheduled_end,
    };
    
    let work_order_id = state.services.work_orders
        .create_work_order(command)
        .await
        .map_err(map_service_error)?;
    
    info\!("Work order created: {}", work_order_id);
    
    created_response(serde_json::json\!({
        "id": work_order_id,
        "message": "Work order created successfully"
    }))
}

/// Get a work order by ID
async fn get_work_order(
    State(state): State<Arc<AppState>>,
    Path(work_order_id): Path<Uuid>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    let work_order = state.services.work_orders
        .get_work_order(&work_order_id)
        .await
        .map_err(map_service_error)?
        .ok_or_else(|| ApiError::NotFound(format\!("Work order with ID {} not found", work_order_id)))?;
    
    success_response(work_order)
}

/// Update a work order
async fn update_work_order(
    State(state): State<Arc<AppState>>,
    Path(work_order_id): Path<Uuid>,
    Json(payload): Json<UpdateWorkOrderRequest>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    validate_input(&payload)?;
    
    let command = UpdateWorkOrderCommand {
        id: work_order_id,
        quantity_planned: payload.quantity_planned,
        priority: payload.priority,
        notes: payload.notes,
        status: payload.status,
    };
    
    state.services.work_orders
        .update_work_order(command)
        .await
        .map_err(map_service_error)?;
    
    info\!("Work order updated: {}", work_order_id);
    
    success_response(serde_json::json\!({
        "message": "Work order updated successfully"
    }))
}

/// Cancel a work order
async fn cancel_work_order(
    State(state): State<Arc<AppState>>,
    Path(work_order_id): Path<Uuid>,
    Json(payload): Json<CancelWorkOrderRequest>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    validate_input(&payload)?;
    
    let command = CancelWorkOrderCommand {
        id: work_order_id,
        reason: payload.reason,
    };
    
    state.services.work_orders
        .cancel_work_order(command)
        .await
        .map_err(map_service_error)?;
    
    info\!("Work order cancelled: {}", work_order_id);
    
    success_response(serde_json::json\!({
        "message": "Work order cancelled successfully"
    }))
}

/// Start a work order
async fn start_work_order(
    State(state): State<Arc<AppState>>,
    Path(work_order_id): Path<Uuid>,
    Json(payload): Json<StartWorkOrderRequest>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    validate_input(&payload)?;
    
    let command = StartWorkOrderCommand {
        id: work_order_id,
        started_by: payload.started_by,
        notes: payload.notes,
    };
    
    state.services.work_orders
        .start_work_order(command)
        .await
        .map_err(map_service_error)?;
    
    info\!("Work order started: {}", work_order_id);
    
    success_response(serde_json::json\!({
        "message": "Work order started successfully"
    }))
}

/// Complete a work order
async fn complete_work_order(
    State(state): State<Arc<AppState>>,
    Path(work_order_id): Path<Uuid>,
    Json(payload): Json<CompleteWorkOrderRequest>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    validate_input(&payload)?;
    
    let command = CompleteWorkOrderCommand {
        id: work_order_id,
        completed_by: payload.completed_by,
        quantity_completed: payload.quantity_completed,
        notes: payload.notes,
    };
    
    state.services.work_orders
        .complete_work_order(command)
        .await
        .map_err(map_service_error)?;
    
    info\!("Work order completed: {}", work_order_id);
    
    success_response(serde_json::json\!({
        "message": "Work order completed successfully"
    }))
}

/// Assign a work order to a user
async fn assign_work_order(
    State(state): State<Arc<AppState>>,
    Path(work_order_id): Path<Uuid>,
    Json(payload): Json<AssignWorkOrderRequest>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    validate_input(&payload)?;
    
    let command = AssignWorkOrderCommand {
        id: work_order_id,
        assigned_to: payload.assigned_to,
        notes: payload.notes,
    };
    
    state.services.work_orders
        .assign_work_order(command)
        .await
        .map_err(map_service_error)?;
    
    info\!("Work order assigned: {} to {}", work_order_id, payload.assigned_to);
    
    success_response(serde_json::json\!({
        "message": "Work order assigned successfully"
    }))
}

/// Unassign a work order
async fn unassign_work_order(
    State(state): State<Arc<AppState>>,
    Path(work_order_id): Path<Uuid>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    let command = UnassignWorkOrderCommand {
        id: work_order_id,
    };
    
    state.services.work_orders
        .unassign_work_order(command)
        .await
        .map_err(map_service_error)?;
    
    info\!("Work order unassigned: {}", work_order_id);
    
    success_response(serde_json::json\!({
        "message": "Work order unassigned successfully"
    }))
}

/// Schedule a work order
async fn schedule_work_order(
    State(state): State<Arc<AppState>>,
    Path(work_order_id): Path<Uuid>,
    Json(payload): Json<ScheduleWorkOrderRequest>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    validate_input(&payload)?;
    
    // Parse dates
    let start_date = NaiveDate::parse_from_str(&payload.start_date, "%Y-%m-%d")
        .map_err(|e| ApiError::BadRequest(format\!("Invalid start date format: {}", e)))?
        .and_hms_opt(0, 0, 0)
        .unwrap();
    
    let end_date = NaiveDate::parse_from_str(&payload.end_date, "%Y-%m-%d")
        .map_err(|e| ApiError::BadRequest(format\!("Invalid end date format: {}", e)))?
        .and_hms_opt(23, 59, 59)
        .unwrap();
    
    let command = ScheduleWorkOrderCommand {
        id: work_order_id,
        start_date,
        end_date,
        notes: payload.notes,
    };
    
    state.services.work_orders
        .schedule_work_order(command)
        .await
        .map_err(map_service_error)?;
    
    info\!("Work order scheduled: {}", work_order_id);
    
    success_response(serde_json::json\!({
        "message": "Work order scheduled successfully"
    }))
}

/// Get work orders by assignee
async fn get_work_orders_by_assignee(
    State(state): State<Arc<AppState>>,
    Path(user_id): Path<Uuid>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    let work_orders = state.services.work_orders
        .get_work_orders_by_assignee(&user_id)
        .await
        .map_err(map_service_error)?;
    
    success_response(work_orders)
}

/// Get work orders by status
async fn get_work_orders_by_status(
    State(state): State<Arc<AppState>>,
    Path(status): Path<String>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    let work_orders = state.services.work_orders
        .get_work_orders_by_status(&status)
        .await
        .map_err(map_service_error)?;
    
    success_response(work_orders)
}

/// Get work orders by schedule
async fn get_work_orders_by_schedule(
    State(state): State<Arc<AppState>>,
    Query(params): Query<DateRangeParams>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    validate_input(&params)?;
    
    let (start_date, end_date) = params.to_datetime_range()?;
    
    let work_orders = state.services.work_orders
        .get_work_orders_by_schedule(start_date, end_date)
        .await
        .map_err(map_service_error)?;
    
    success_response(work_orders)
}

