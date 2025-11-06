use crate::errors::ServiceError;
use crate::models::work_order::{
    Model as WorkOrderModel, WorkOrderPriority as ModelPriority, WorkOrderStatus as ModelStatus,
};
use crate::services::work_orders::{WorkOrderCreateData, WorkOrderService, WorkOrderUpdateData};
use crate::{ApiResponse, PaginatedResponse};
use axum::{
    extract::{Json, Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post, put},
    Router,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

// State trait that exposes the work order service
pub trait WorkOrdersHandlerState: Clone + Send + Sync + 'static {
    fn work_orders_service(&self) -> Arc<WorkOrderService>;
}

impl WorkOrdersHandlerState for crate::AppState {
    fn work_orders_service(&self) -> Arc<WorkOrderService> {
        self.work_order_service()
    }
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct WorkOrder {
    pub id: String,
    pub order_id: Option<String>,
    pub product_id: String,
    pub bom_id: Option<String>,
    pub quantity: i32,
    pub priority: String, // "low", "normal", "high", "urgent"
    pub status: String, // "planned", "scheduled", "in_progress", "on_hold", "completed", "cancelled"
    pub work_center_id: String,
    pub assigned_to: Option<String>,
    pub scheduled_start: Option<DateTime<Utc>>,
    pub scheduled_end: Option<DateTime<Utc>>,
    pub actual_start: Option<DateTime<Utc>>,
    pub actual_end: Option<DateTime<Utc>>,
    pub estimated_hours: Option<f64>,
    pub actual_hours: Option<f64>,
    pub materials: Vec<WorkOrderMaterial>,
    pub tasks: Vec<WorkOrderTask>,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct WorkOrderMaterial {
    pub id: String,
    pub material_id: String,
    pub material_name: String,
    pub required_quantity: f64,
    pub allocated_quantity: f64,
    pub consumed_quantity: f64,
    pub unit_of_measure: String,
    pub cost_per_unit: Option<f64>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct WorkOrderTask {
    pub id: String,
    pub task_name: String,
    pub description: Option<String>,
    pub sequence: i32,
    pub status: String, // "pending", "in_progress", "completed", "skipped"
    pub estimated_hours: Option<f64>,
    pub actual_hours: Option<f64>,
    pub assigned_to: Option<String>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateWorkOrderRequest {
    pub title: String,
    pub description: Option<String>,
    pub status: Option<String>,
    pub priority: Option<String>,
    pub asset_id: Option<Uuid>,
    pub assigned_to: Option<Uuid>,
    pub due_date: Option<DateTime<Utc>>,
    pub bill_of_materials_number: Option<String>,
    pub quantity_produced: Option<i32>,
    #[schema(value_type = Object)]
    pub parts_required: Option<Value>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateWorkOrderRequest {
    pub title: Option<String>,
    pub description: Option<String>,
    pub status: Option<String>,
    pub priority: Option<String>,
    pub asset_id: Option<Uuid>,
    pub assigned_to: Option<Uuid>,
    pub due_date: Option<DateTime<Utc>>,
    pub bill_of_materials_number: Option<String>,
    pub quantity_produced: Option<i32>,
    #[schema(value_type = Object)]
    pub parts_required: Option<Value>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ScheduleWorkOrderRequest {
    pub work_center_id: String,
    pub scheduled_start: DateTime<Utc>,
    pub scheduled_end: DateTime<Utc>,
    pub assigned_to: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct MaterialConsumptionRequest {
    pub material_id: String,
    pub quantity_consumed: f64,
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct TaskUpdateRequest {
    pub status: Option<String>,
    pub actual_hours: Option<f64>,
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct WorkOrderFilters {
    pub status: Option<String>,
    pub priority: Option<String>,
    pub work_center_id: Option<String>,
    pub assigned_to: Option<String>,
    pub product_id: Option<String>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

/// API representation of a work order persisted in the database.
#[derive(Debug, Serialize, ToSchema)]
pub struct WorkOrderResponse {
    pub id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub status: String,
    pub priority: String,
    pub asset_id: Option<Uuid>,
    pub assigned_to: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub due_date: Option<DateTime<Utc>>,
    pub bill_of_materials_number: Option<String>,
    pub quantity_produced: Option<i32>,
    #[schema(value_type = Object)]
    pub parts_required: Value,
}

fn map_work_order(model: WorkOrderModel) -> WorkOrderResponse {
    WorkOrderResponse {
        id: model.id,
        title: model.title,
        description: model.description,
        status: status_to_string(model.status),
        priority: priority_to_string(model.priority),
        asset_id: model.asset_id,
        assigned_to: model.assigned_to,
        created_at: model.created_at,
        updated_at: model.updated_at,
        due_date: model.due_date,
        bill_of_materials_number: model.bill_of_materials_number,
        quantity_produced: model.quantity_produced,
        parts_required: model.parts_required,
    }
}

fn status_to_string(status: ModelStatus) -> String {
    match status {
        ModelStatus::Pending => "pending",
        ModelStatus::InProgress => "in_progress",
        ModelStatus::Completed => "completed",
        ModelStatus::Cancelled => "cancelled",
        ModelStatus::Issued => "issued",
        ModelStatus::Picked => "picked",
        ModelStatus::Yielded => "yielded",
    }
    .to_string()
}

fn priority_to_string(priority: ModelPriority) -> String {
    match priority {
        ModelPriority::Low => "low",
        ModelPriority::Normal => "normal",
        ModelPriority::High => "high",
        ModelPriority::Urgent => "urgent",
    }
    .to_string()
}

fn parse_priority_default(value: Option<&str>) -> Result<ModelPriority, ServiceError> {
    match value {
        Some(raw) => parse_priority_value(raw),
        None => Ok(ModelPriority::Normal),
    }
}

fn parse_priority_optional(value: Option<&str>) -> Result<Option<ModelPriority>, ServiceError> {
    value.map(parse_priority_value).transpose()
}

fn parse_priority_value(value: &str) -> Result<ModelPriority, ServiceError> {
    match value.to_ascii_lowercase().as_str() {
        "low" => Ok(ModelPriority::Low),
        "normal" => Ok(ModelPriority::Normal),
        "high" => Ok(ModelPriority::High),
        "urgent" => Ok(ModelPriority::Urgent),
        other => Err(ServiceError::ValidationError(format!(
            "Invalid priority: {}",
            other
        ))),
    }
}

fn parse_status_default(value: Option<&str>) -> Result<ModelStatus, ServiceError> {
    match value {
        Some(raw) => parse_status_value(raw),
        None => Ok(ModelStatus::Pending),
    }
}

fn parse_status_optional(value: Option<&str>) -> Result<Option<ModelStatus>, ServiceError> {
    value.map(parse_status_value).transpose()
}

fn parse_status_value(value: &str) -> Result<ModelStatus, ServiceError> {
    match value.to_ascii_lowercase().as_str() {
        "pending" => Ok(ModelStatus::Pending),
        "in_progress" | "in-progress" | "inprogress" => Ok(ModelStatus::InProgress),
        "completed" => Ok(ModelStatus::Completed),
        "cancelled" | "canceled" => Ok(ModelStatus::Cancelled),
        "issued" => Ok(ModelStatus::Issued),
        "picked" => Ok(ModelStatus::Picked),
        "yielded" => Ok(ModelStatus::Yielded),
        other => Err(ServiceError::ValidationError(format!(
            "Invalid status: {}",
            other
        ))),
    }
}

/// Create the work orders router
pub fn work_orders_router<S>() -> Router<S>
where
    S: WorkOrdersHandlerState,
{
    Router::new()
        .route("/", get(list_work_orders::<S>).post(create_work_order::<S>))
        .route(
            "/{id}",
            get(get_work_order::<S>)
                .put(update_work_order::<S>)
                .delete(delete_work_order::<S>),
        )
        .route("/{id}/schedule", post(schedule_work_order::<S>))
        .route("/{id}/start", post(start_work_order::<S>))
        .route("/{id}/complete", post(complete_work_order::<S>))
        .route("/{id}/hold", post(hold_work_order::<S>))
        .route("/{id}/cancel", post(cancel_work_order::<S>))
        .route(
            "/{id}/materials/{material_id}/consume",
            post(consume_material::<S>),
        )
        .route("/{id}/tasks/{task_id}", put(update_task::<S>))
        .route("/capacity/:work_center_id", get(get_capacity::<S>))
}

/// List work orders with optional filtering
#[utoipa::path(
    get,
    path = "/api/v1/work-orders",
    params(WorkOrderFilters),
    responses(
        (status = 200, description = "List work orders", body = crate::ApiResponse<crate::PaginatedResponse<WorkOrderResponse>>,
            headers(
                ("X-Request-Id" = String, description = "Unique request id"),
                ("X-RateLimit-Limit" = String, description = "Requests allowed in current window"),
                ("X-RateLimit-Remaining" = String, description = "Remaining requests in window"),
                ("X-RateLimit-Reset" = String, description = "Seconds until reset"),
            )
        ),
        (status = 401, description = "Unauthorized", body = crate::errors::ErrorResponse),
        (status = 403, description = "Forbidden", body = crate::errors::ErrorResponse)
    ),
    tag = "work-orders"
)]
pub async fn list_work_orders<S>(
    State(state): State<S>,
    Query(filters): Query<WorkOrderFilters>,
) -> Result<Json<ApiResponse<PaginatedResponse<WorkOrderResponse>>>, ServiceError>
where
    S: WorkOrdersHandlerState,
{
    let WorkOrderFilters {
        status,
        limit,
        offset,
        ..
    } = filters;

    let per_page = limit.unwrap_or(20).max(1).min(100) as u64;
    let offset = offset.unwrap_or(0) as u64;
    let page = offset / per_page + 1;

    let service = state.work_orders_service();

    let (models, total) = if let Some(status) = status {
        service
            .get_work_orders_by_status(&status, page, per_page)
            .await?
    } else {
        service.list_work_orders(page, per_page).await?
    };

    let items = models.into_iter().map(map_work_order).collect::<Vec<_>>();
    let total_pages = if total == 0 {
        0
    } else {
        (total + per_page - 1) / per_page
    };

    let response = PaginatedResponse {
        items,
        total,
        page,
        limit: per_page,
        total_pages,
    };

    Ok(Json(ApiResponse::success(response)))
}

/// Create a new work order
#[utoipa::path(
    post,
    path = "/api/v1/work-orders",
    request_body = CreateWorkOrderRequest,
    responses(
        (status = 201, description = "Work order created", body = crate::ApiResponse<WorkOrderResponse>,
            headers(("X-Request-Id" = String, description = "Unique request id"))
        ),
        (status = 400, description = "Invalid request", body = crate::errors::ErrorResponse),
        (status = 401, description = "Unauthorized", body = crate::errors::ErrorResponse),
        (status = 403, description = "Forbidden", body = crate::errors::ErrorResponse)
    ),
    tag = "work-orders"
)]
pub async fn create_work_order<S>(
    State(state): State<S>,
    Json(payload): Json<CreateWorkOrderRequest>,
) -> Result<(StatusCode, Json<ApiResponse<WorkOrderResponse>>), ServiceError>
where
    S: WorkOrdersHandlerState,
{
    if payload.title.trim().is_empty() {
        return Err(ServiceError::ValidationError(
            "title cannot be empty".to_string(),
        ));
    }

    let priority = parse_priority_default(payload.priority.as_deref())?;
    let status = parse_status_default(payload.status.as_deref())?;

    let data = WorkOrderCreateData {
        title: payload.title.clone(),
        description: payload.description.clone(),
        status: Some(status),
        priority,
        asset_id: payload.asset_id,
        assigned_to: payload.assigned_to,
        due_date: payload.due_date,
        bill_of_materials_number: payload.bill_of_materials_number.clone(),
        quantity_produced: payload.quantity_produced,
        parts_required: payload.parts_required.clone(),
    };

    let created = state.work_orders_service().create_work_order(data).await?;

    Ok((
        StatusCode::CREATED,
        Json(ApiResponse::success(map_work_order(created))),
    ))
}

/// Get a specific work order by ID
#[utoipa::path(
    get,
    path = "/api/v1/work-orders/{id}",
    params(("id" = String, Path, description = "Work order ID")),
    responses(
        (status = 200, description = "Work order details", body = crate::ApiResponse<WorkOrderResponse>,
            headers(("X-Request-Id" = String, description = "Unique request id"))
        ),
        (status = 401, description = "Unauthorized", body = crate::errors::ErrorResponse),
        (status = 403, description = "Forbidden", body = crate::errors::ErrorResponse),
        (status = 404, description = "Not found", body = crate::errors::ErrorResponse)
    ),
    tag = "work-orders"
)]
pub async fn get_work_order<S>(
    State(state): State<S>,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<WorkOrderResponse>>, ServiceError>
where
    S: WorkOrdersHandlerState,
{
    let service = state.work_orders_service();
    let work_order = service
        .get_work_order(&id)
        .await?
        .ok_or_else(|| ServiceError::NotFound(format!("Work order {} not found", id)))?;

    Ok(Json(ApiResponse::success(map_work_order(work_order))))
}

/// Update a work order
#[utoipa::path(
    put,
    path = "/api/v1/work-orders/{id}",
    params(("id" = String, Path, description = "Work order ID")),
    request_body = UpdateWorkOrderRequest,
    responses(
        (status = 200, description = "Work order updated", body = crate::ApiResponse<WorkOrderResponse>,
            headers(("X-Request-Id" = String, description = "Unique request id"))
        ),
        (status = 400, description = "Invalid request", body = crate::errors::ErrorResponse),
        (status = 401, description = "Unauthorized", body = crate::errors::ErrorResponse),
        (status = 403, description = "Forbidden", body = crate::errors::ErrorResponse),
        (status = 404, description = "Not found", body = crate::errors::ErrorResponse)
    ),
    tag = "work-orders"
)]
pub async fn update_work_order<S>(
    State(state): State<S>,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateWorkOrderRequest>,
) -> Result<Json<ApiResponse<WorkOrderResponse>>, ServiceError>
where
    S: WorkOrdersHandlerState,
{
    let status = parse_status_optional(payload.status.as_deref())?;
    let priority = parse_priority_optional(payload.priority.as_deref())?;

    let data = WorkOrderUpdateData {
        title: payload.title.clone(),
        description: payload.description.clone(),
        status,
        priority,
        asset_id: payload.asset_id,
        assigned_to: payload.assigned_to,
        due_date: payload.due_date,
        bill_of_materials_number: payload.bill_of_materials_number.clone(),
        quantity_produced: payload.quantity_produced,
        parts_required: payload.parts_required.clone(),
    };

    let updated = state
        .work_orders_service()
        .update_work_order(&id, data)
        .await?;

    Ok(Json(ApiResponse::success(map_work_order(updated))))
}

/// Delete a work order
#[utoipa::path(
    delete,
    path = "/api/v1/work-orders/{id}",
    params(("id" = String, Path, description = "Work order ID")),
    responses(
        (status = 204, description = "Work order deleted"),
        (status = 401, description = "Unauthorized", body = crate::errors::ErrorResponse),
        (status = 403, description = "Forbidden", body = crate::errors::ErrorResponse),
        (status = 404, description = "Not found", body = crate::errors::ErrorResponse)
    ),
    tag = "work-orders"
)]
pub async fn delete_work_order<S>(
    State(state): State<S>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ServiceError>
where
    S: WorkOrdersHandlerState,
{
    state.work_orders_service().delete_work_order(&id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Schedule a work order
#[utoipa::path(
    post,
    path = "/api/v1/work-orders/{id}/schedule",
    params(("id" = String, Path, description = "Work order ID")),
    request_body = ScheduleWorkOrderRequest,
    responses(
        (status = 200, description = "Work order scheduled",
            headers(("X-Request-Id" = String, description = "Unique request id"))
        ),
        (status = 400, description = "Invalid request", body = crate::errors::ErrorResponse),
        (status = 401, description = "Unauthorized", body = crate::errors::ErrorResponse),
        (status = 403, description = "Forbidden", body = crate::errors::ErrorResponse)
    ),
    tag = "work-orders"
)]
pub async fn schedule_work_order<S>(
    State(_state): State<S>,
    Path(id): Path<String>,
    Json(payload): Json<ScheduleWorkOrderRequest>,
) -> Result<impl IntoResponse, ServiceError>
where
    S: WorkOrdersHandlerState,
{
    let response = json!({
        "message": format!("Work order {} has been scheduled", id),
        "work_order_id": id,
        "status": "scheduled",
        "work_center_id": payload.work_center_id,
        "scheduled_start": payload.scheduled_start,
        "scheduled_end": payload.scheduled_end,
        "assigned_to": payload.assigned_to,
        "scheduled_at": Utc::now()
    });

    Ok((StatusCode::OK, Json(response)))
}

/// Start a work order
#[utoipa::path(
    post,
    path = "/api/v1/work-orders/{id}/start",
    params(("id" = String, Path, description = "Work order ID")),
    responses(
        (status = 200, description = "Work order started",
            headers(("X-Request-Id" = String, description = "Unique request id"))
        ),
        (status = 401, description = "Unauthorized", body = crate::errors::ErrorResponse),
        (status = 403, description = "Forbidden", body = crate::errors::ErrorResponse)
    ),
    tag = "work-orders"
)]
pub async fn start_work_order<S>(
    State(_state): State<S>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, ServiceError>
where
    S: WorkOrdersHandlerState,
{
    let response = json!({
        "message": format!("Work order {} has been started", id),
        "work_order_id": id,
        "status": "in_progress",
        "actual_start": Utc::now()
    });

    Ok((StatusCode::OK, Json(response)))
}

/// Complete a work order
#[utoipa::path(
    post,
    path = "/api/v1/work-orders/{id}/complete",
    params(("id" = String, Path, description = "Work order ID")),
    responses(
        (status = 200, description = "Work order completed",
            headers(("X-Request-Id" = String, description = "Unique request id"))
        ),
        (status = 401, description = "Unauthorized", body = crate::errors::ErrorResponse),
        (status = 403, description = "Forbidden", body = crate::errors::ErrorResponse)
    ),
    tag = "work-orders"
)]
pub async fn complete_work_order<S>(
    State(_state): State<S>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, ServiceError>
where
    S: WorkOrdersHandlerState,
{
    let response = json!({
        "message": format!("Work order {} has been completed", id),
        "work_order_id": id,
        "status": "completed",
        "actual_end": Utc::now()
    });

    Ok((StatusCode::OK, Json(response)))
}

/// Put work order on hold
#[utoipa::path(
    post,
    path = "/api/v1/work-orders/{id}/hold",
    params(("id" = String, Path, description = "Work order ID")),
    responses(
        (status = 200, description = "Work order on hold",
            headers(("X-Request-Id" = String, description = "Unique request id"))
        ),
        (status = 401, description = "Unauthorized", body = crate::errors::ErrorResponse),
        (status = 403, description = "Forbidden", body = crate::errors::ErrorResponse)
    ),
    tag = "work-orders"
)]
pub async fn hold_work_order<S>(
    State(_state): State<S>,
    Path(id): Path<String>,
    Json(payload): Json<serde_json::Value>,
) -> Result<impl IntoResponse, ServiceError>
where
    S: WorkOrdersHandlerState,
{
    let reason = payload
        .get("reason")
        .and_then(|v| v.as_str())
        .unwrap_or("No reason provided");

    let response = json!({
        "message": format!("Work order {} has been put on hold", id),
        "work_order_id": id,
        "status": "on_hold",
        "reason": reason,
        "held_at": Utc::now()
    });

    Ok((StatusCode::OK, Json(response)))
}

/// Cancel a work order
#[utoipa::path(
    post,
    path = "/api/v1/work-orders/{id}/cancel",
    params(("id" = String, Path, description = "Work order ID")),
    responses(
        (status = 200, description = "Work order cancelled",
            headers(("X-Request-Id" = String, description = "Unique request id"))
        ),
        (status = 401, description = "Unauthorized", body = crate::errors::ErrorResponse),
        (status = 403, description = "Forbidden", body = crate::errors::ErrorResponse)
    ),
    tag = "work-orders"
)]
pub async fn cancel_work_order<S>(
    State(_state): State<S>,
    Path(id): Path<String>,
    Json(payload): Json<serde_json::Value>,
) -> Result<impl IntoResponse, ServiceError>
where
    S: WorkOrdersHandlerState,
{
    let reason = payload
        .get("reason")
        .and_then(|v| v.as_str())
        .unwrap_or("No reason provided");

    let response = json!({
        "message": format!("Work order {} has been cancelled", id),
        "work_order_id": id,
        "status": "cancelled",
        "reason": reason,
        "cancelled_at": Utc::now()
    });

    Ok((StatusCode::OK, Json(response)))
}

/// Consume material for work order
#[utoipa::path(
    post,
    path = "/api/v1/work-orders/{id}/materials/{material_id}/consume",
    params(
        ("id" = String, Path, description = "Work order ID"),
        ("material_id" = String, Path, description = "Material ID")
    ),
    request_body = MaterialConsumptionRequest,
    responses(
        (status = 200, description = "Material consumed",
            headers(("X-Request-Id" = String, description = "Unique request id"))
        ),
        (status = 400, description = "Invalid request", body = crate::errors::ErrorResponse),
        (status = 401, description = "Unauthorized", body = crate::errors::ErrorResponse),
        (status = 403, description = "Forbidden", body = crate::errors::ErrorResponse)
    ),
    tag = "work-orders"
)]
pub async fn consume_material<S>(
    State(_state): State<S>,
    Path((id, material_id)): Path<(String, String)>,
    Json(payload): Json<MaterialConsumptionRequest>,
) -> Result<impl IntoResponse, ServiceError>
where
    S: WorkOrdersHandlerState,
{
    let response = json!({
        "message": format!("Material {} consumed for work order {}", material_id, id),
        "work_order_id": id,
        "material_id": material_id,
        "quantity_consumed": payload.quantity_consumed,
        "consumed_at": Utc::now(),
        "notes": payload.notes
    });

    Ok((StatusCode::OK, Json(response)))
}

/// Update a work order task
#[utoipa::path(
    put,
    path = "/api/v1/work-orders/{id}/tasks/{task_id}",
    params(
        ("id" = String, Path, description = "Work order ID"),
        ("task_id" = String, Path, description = "Task ID")
    ),
    request_body = TaskUpdateRequest,
    responses(
        (status = 200, description = "Task updated",
            headers(("X-Request-Id" = String, description = "Unique request id"))
        ),
        (status = 400, description = "Invalid request", body = crate::errors::ErrorResponse),
        (status = 401, description = "Unauthorized", body = crate::errors::ErrorResponse),
        (status = 403, description = "Forbidden", body = crate::errors::ErrorResponse)
    ),
    tag = "work-orders"
)]
pub async fn update_task<S>(
    State(_state): State<S>,
    Path((id, task_id)): Path<(String, String)>,
    Json(payload): Json<TaskUpdateRequest>,
) -> Result<impl IntoResponse, ServiceError>
where
    S: WorkOrdersHandlerState,
{
    let response = json!({
        "message": format!("Task {} updated for work order {}", task_id, id),
        "work_order_id": id,
        "task_id": task_id,
        "status": payload.status,
        "actual_hours": payload.actual_hours,
        "notes": payload.notes,
        "updated_at": Utc::now()
    });

    Ok((StatusCode::OK, Json(response)))
}

/// Get work center capacity
#[utoipa::path(
    get,
    path = "/api/v1/work-orders/capacity/{work_center_id}",
    params(("work_center_id" = String, Path, description = "Work center ID")),
    responses(
        (status = 200, description = "Capacity details",
            headers(
                ("X-Request-Id" = String, description = "Unique request id"),
                ("X-RateLimit-Limit" = String, description = "Requests allowed in current window"),
                ("X-RateLimit-Remaining" = String, description = "Remaining requests in window"),
                ("X-RateLimit-Reset" = String, description = "Seconds until reset"),
            )
        ),
        (status = 401, description = "Unauthorized", body = crate::errors::ErrorResponse)
    ),
    tag = "work-orders"
)]
pub async fn get_capacity<S>(
    State(_state): State<S>,
    Path(work_center_id): Path<String>,
    Query(filters): Query<serde_json::Value>,
) -> Result<impl IntoResponse, ServiceError>
where
    S: WorkOrdersHandlerState,
{
    let start_date = filters
        .get("start_date")
        .and_then(|v| v.as_str())
        .unwrap_or("2024-01-01");
    let end_date = filters
        .get("end_date")
        .and_then(|v| v.as_str())
        .unwrap_or("2024-12-31");

    // Mock capacity data
    let capacity_data = json!({
        "work_center_id": work_center_id,
        "period": {
            "start_date": start_date,
            "end_date": end_date
        },
        "total_capacity_hours": 2080.0, // 52 weeks * 40 hours
        "scheduled_hours": 1560.0,
        "available_hours": 520.0,
        "utilization_percentage": 75.0,
        "work_orders_scheduled": 15,
        "bottleneck_periods": [
            {
                "start": "2024-06-01",
                "end": "2024-06-15",
                "utilization": 95.0
            }
        ]
    });

    Ok((StatusCode::OK, Json(capacity_data)))
}

/// Assign a work order to a technician
#[utoipa::path(
    post,
    path = "/api/v1/work-orders/{id}/assign",
    params(("id" = String, Path, description = "Work order ID")),
    request_body = serde_json::Value,
    responses(
        (status = 200, description = "Work order assigned", body = serde_json::Value),
        (status = 401, description = "Unauthorized", body = crate::errors::ErrorResponse),
        (status = 403, description = "Forbidden", body = crate::errors::ErrorResponse)
    ),
    tag = "work-orders"
)]
pub async fn assign_work_order<S>(
    State(_state): State<S>,
    Path(id): Path<String>,
    Json(payload): Json<serde_json::Value>,
) -> Result<impl IntoResponse, ServiceError>
where
    S: WorkOrdersHandlerState,
{
    let response = json!({
        "message": format!("Work order {} has been assigned", id),
        "work_order_id": id,
        "assigned_to": payload.get("technician_id").unwrap_or(&json!("technician-123")),
        "assigned_at": Utc::now()
    });

    Ok((StatusCode::OK, Json(response)))
}

/// Update work order status
#[utoipa::path(
    put,
    path = "/api/v1/work-orders/{id}/status",
    params(("id" = String, Path, description = "Work order ID")),
    request_body = serde_json::Value,
    responses(
        (status = 200, description = "Work order status updated", body = serde_json::Value),
        (status = 401, description = "Unauthorized", body = crate::errors::ErrorResponse),
        (status = 403, description = "Forbidden", body = crate::errors::ErrorResponse)
    ),
    tag = "work-orders"
)]
pub async fn update_work_order_status<S>(
    State(_state): State<S>,
    Path(id): Path<String>,
    Json(payload): Json<serde_json::Value>,
) -> Result<impl IntoResponse, ServiceError>
where
    S: WorkOrdersHandlerState,
{
    let new_status = payload
        .get("status")
        .and_then(|s| s.as_str())
        .unwrap_or("unknown");

    let response = json!({
        "message": format!("Work order {} status updated to {}", id, new_status),
        "work_order_id": id,
        "status": new_status,
        "updated_at": Utc::now()
    });

    Ok((StatusCode::OK, Json(response)))
}
