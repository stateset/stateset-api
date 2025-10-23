use crate::errors::ServiceError;
use axum::{
    extract::{Json, Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post, put},
    Router,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

// Generic trait for work orders handler state
pub trait WorkOrdersAppState: Clone + Send + Sync + 'static {}
impl<T> WorkOrdersAppState for T where T: Clone + Send + Sync + 'static {}

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
    pub order_id: Option<String>,
    pub product_id: String,
    pub bom_id: Option<String>,
    pub quantity: i32,
    pub priority: String,
    pub work_center_id: String,
    pub scheduled_start: Option<DateTime<Utc>>,
    pub scheduled_end: Option<DateTime<Utc>>,
    pub estimated_hours: Option<f64>,
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateWorkOrderRequest {
    pub status: Option<String>,
    pub priority: Option<String>,
    pub assigned_to: Option<String>,
    pub scheduled_start: Option<DateTime<Utc>>,
    pub scheduled_end: Option<DateTime<Utc>>,
    pub estimated_hours: Option<f64>,
    pub notes: Option<String>,
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

/// Create the work orders router
pub fn work_orders_router<S>() -> Router<S>
where
    S: WorkOrdersAppState,
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
        (status = 200, description = "List work orders",
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
    State(_state): State<S>,
    Query(filters): Query<WorkOrderFilters>,
) -> Result<impl IntoResponse, ServiceError>
where
    S: WorkOrdersAppState,
{
    // Mock data for now - replace with actual database queries
    let mut work_orders = vec![
        WorkOrder {
            id: "wo_001".to_string(),
            order_id: Some("order_001".to_string()),
            product_id: "prod_abc".to_string(),
            bom_id: Some("bom_001".to_string()),
            quantity: 100,
            priority: "high".to_string(),
            status: "in_progress".to_string(),
            work_center_id: "wc_assembly".to_string(),
            assigned_to: Some("worker_001".to_string()),
            scheduled_start: Some(Utc::now() - chrono::Duration::hours(4)),
            scheduled_end: Some(Utc::now() + chrono::Duration::hours(4)),
            actual_start: Some(Utc::now() - chrono::Duration::hours(2)),
            actual_end: None,
            estimated_hours: Some(8.0),
            actual_hours: Some(2.0),
            materials: vec![WorkOrderMaterial {
                id: "wom_001".to_string(),
                material_id: "mat_001".to_string(),
                material_name: "Steel Frame".to_string(),
                required_quantity: 100.0,
                allocated_quantity: 100.0,
                consumed_quantity: 50.0,
                unit_of_measure: "pieces".to_string(),
                cost_per_unit: Some(15.50),
            }],
            tasks: vec![WorkOrderTask {
                id: "wot_001".to_string(),
                task_name: "Assembly".to_string(),
                description: Some("Assemble main frame".to_string()),
                sequence: 1,
                status: "in_progress".to_string(),
                estimated_hours: Some(4.0),
                actual_hours: Some(2.0),
                assigned_to: Some("worker_001".to_string()),
                started_at: Some(Utc::now() - chrono::Duration::hours(2)),
                completed_at: None,
                notes: None,
            }],
            notes: Some("High priority customer order".to_string()),
            created_at: Utc::now() - chrono::Duration::days(1),
            updated_at: Utc::now() - chrono::Duration::hours(1),
        },
        WorkOrder {
            id: "wo_002".to_string(),
            order_id: None,
            product_id: "prod_def".to_string(),
            bom_id: Some("bom_002".to_string()),
            quantity: 50,
            priority: "normal".to_string(),
            status: "scheduled".to_string(),
            work_center_id: "wc_painting".to_string(),
            assigned_to: Some("worker_002".to_string()),
            scheduled_start: Some(Utc::now() + chrono::Duration::hours(8)),
            scheduled_end: Some(Utc::now() + chrono::Duration::hours(16)),
            actual_start: None,
            actual_end: None,
            estimated_hours: Some(8.0),
            actual_hours: None,
            materials: vec![WorkOrderMaterial {
                id: "wom_002".to_string(),
                material_id: "mat_002".to_string(),
                material_name: "Paint - Blue".to_string(),
                required_quantity: 10.0,
                allocated_quantity: 10.0,
                consumed_quantity: 0.0,
                unit_of_measure: "liters".to_string(),
                cost_per_unit: Some(25.00),
            }],
            tasks: vec![
                WorkOrderTask {
                    id: "wot_002".to_string(),
                    task_name: "Surface Preparation".to_string(),
                    description: Some("Clean and prime surface".to_string()),
                    sequence: 1,
                    status: "pending".to_string(),
                    estimated_hours: Some(2.0),
                    actual_hours: None,
                    assigned_to: Some("worker_002".to_string()),
                    started_at: None,
                    completed_at: None,
                    notes: None,
                },
                WorkOrderTask {
                    id: "wot_003".to_string(),
                    task_name: "Paint Application".to_string(),
                    description: Some("Apply paint coating".to_string()),
                    sequence: 2,
                    status: "pending".to_string(),
                    estimated_hours: Some(6.0),
                    actual_hours: None,
                    assigned_to: Some("worker_002".to_string()),
                    started_at: None,
                    completed_at: None,
                    notes: None,
                },
            ],
            notes: None,
            created_at: Utc::now() - chrono::Duration::hours(12),
            updated_at: Utc::now() - chrono::Duration::hours(8),
        },
    ];

    // Apply filters
    if let Some(status) = &filters.status {
        work_orders.retain(|wo| &wo.status == status);
    }
    if let Some(priority) = &filters.priority {
        work_orders.retain(|wo| &wo.priority == priority);
    }
    if let Some(work_center_id) = &filters.work_center_id {
        work_orders.retain(|wo| &wo.work_center_id == work_center_id);
    }
    if let Some(assigned_to) = &filters.assigned_to {
        work_orders.retain(|wo| wo.assigned_to.as_ref() == Some(assigned_to));
    }
    if let Some(product_id) = &filters.product_id {
        work_orders.retain(|wo| &wo.product_id == product_id);
    }

    let response = json!({
        "work_orders": work_orders,
        "total": work_orders.len(),
        "limit": filters.limit.unwrap_or(50),
        "offset": filters.offset.unwrap_or(0)
    });

    Ok((StatusCode::OK, Json(response)))
}

/// Create a new work order
#[utoipa::path(
    post,
    path = "/api/v1/work-orders",
    request_body = CreateWorkOrderRequest,
    responses(
        (status = 201, description = "Work order created", body = WorkOrder,
            headers(("X-Request-Id" = String, description = "Unique request id"))
        ),
        (status = 400, description = "Invalid request", body = crate::errors::ErrorResponse),
        (status = 401, description = "Unauthorized", body = crate::errors::ErrorResponse),
        (status = 403, description = "Forbidden", body = crate::errors::ErrorResponse)
    ),
    tag = "work-orders"
)]
pub async fn create_work_order<S>(
    State(_state): State<S>,
    Json(payload): Json<CreateWorkOrderRequest>,
) -> Result<impl IntoResponse, ServiceError>
where
    S: WorkOrdersAppState,
{
    let work_order_id = Uuid::new_v4().to_string();
    let now = Utc::now();

    let work_order = WorkOrder {
        id: work_order_id.clone(),
        order_id: payload.order_id,
        product_id: payload.product_id,
        bom_id: payload.bom_id,
        quantity: payload.quantity,
        priority: payload.priority,
        status: "planned".to_string(),
        work_center_id: payload.work_center_id,
        assigned_to: None,
        scheduled_start: payload.scheduled_start,
        scheduled_end: payload.scheduled_end,
        actual_start: None,
        actual_end: None,
        estimated_hours: payload.estimated_hours,
        actual_hours: None,
        materials: vec![], // Will be populated from BOM
        tasks: vec![],     // Will be created based on routing
        notes: payload.notes,
        created_at: now,
        updated_at: now,
    };

    Ok((StatusCode::CREATED, Json(work_order)))
}

/// Get a specific work order by ID
#[utoipa::path(
    get,
    path = "/api/v1/work-orders/{id}",
    params(("id" = String, Path, description = "Work order ID")),
    responses(
        (status = 200, description = "Work order details", body = WorkOrder,
            headers(("X-Request-Id" = String, description = "Unique request id"))
        ),
        (status = 401, description = "Unauthorized", body = crate::errors::ErrorResponse),
        (status = 403, description = "Forbidden", body = crate::errors::ErrorResponse),
        (status = 404, description = "Not found", body = crate::errors::ErrorResponse)
    ),
    tag = "work-orders"
)]
pub async fn get_work_order<S>(
    State(_state): State<S>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, ServiceError>
where
    S: WorkOrdersAppState,
{
    let work_order = WorkOrder {
        id: id.clone(),
        order_id: Some("order_001".to_string()),
        product_id: "prod_abc".to_string(),
        bom_id: Some("bom_001".to_string()),
        quantity: 100,
        priority: "high".to_string(),
        status: "in_progress".to_string(),
        work_center_id: "wc_assembly".to_string(),
        assigned_to: Some("worker_001".to_string()),
        scheduled_start: Some(Utc::now() - chrono::Duration::hours(4)),
        scheduled_end: Some(Utc::now() + chrono::Duration::hours(4)),
        actual_start: Some(Utc::now() - chrono::Duration::hours(2)),
        actual_end: None,
        estimated_hours: Some(8.0),
        actual_hours: Some(2.0),
        materials: vec![WorkOrderMaterial {
            id: "wom_001".to_string(),
            material_id: "mat_001".to_string(),
            material_name: "Steel Frame".to_string(),
            required_quantity: 100.0,
            allocated_quantity: 100.0,
            consumed_quantity: 50.0,
            unit_of_measure: "pieces".to_string(),
            cost_per_unit: Some(15.50),
        }],
        tasks: vec![WorkOrderTask {
            id: "wot_001".to_string(),
            task_name: "Assembly".to_string(),
            description: Some("Assemble main frame".to_string()),
            sequence: 1,
            status: "in_progress".to_string(),
            estimated_hours: Some(4.0),
            actual_hours: Some(2.0),
            assigned_to: Some("worker_001".to_string()),
            started_at: Some(Utc::now() - chrono::Duration::hours(2)),
            completed_at: None,
            notes: None,
        }],
        notes: Some("High priority customer order".to_string()),
        created_at: Utc::now() - chrono::Duration::days(1),
        updated_at: Utc::now() - chrono::Duration::hours(1),
    };

    Ok((StatusCode::OK, Json(work_order)))
}

/// Update a work order
#[utoipa::path(
    put,
    path = "/api/v1/work-orders/{id}",
    params(("id" = String, Path, description = "Work order ID")),
    request_body = UpdateWorkOrderRequest,
    responses(
        (status = 200, description = "Work order updated", body = WorkOrder,
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
    State(_state): State<S>,
    Path(id): Path<String>,
    Json(payload): Json<UpdateWorkOrderRequest>,
) -> Result<impl IntoResponse, ServiceError>
where
    S: WorkOrdersAppState,
{
    let work_order = WorkOrder {
        id: id.clone(),
        order_id: Some("order_001".to_string()),
        product_id: "prod_abc".to_string(),
        bom_id: Some("bom_001".to_string()),
        quantity: 100,
        priority: payload.priority.unwrap_or_else(|| "high".to_string()),
        status: payload.status.unwrap_or_else(|| "in_progress".to_string()),
        work_center_id: "wc_assembly".to_string(),
        assigned_to: payload.assigned_to.or(Some("worker_001".to_string())),
        scheduled_start: payload
            .scheduled_start
            .or(Some(Utc::now() - chrono::Duration::hours(4))),
        scheduled_end: payload
            .scheduled_end
            .or(Some(Utc::now() + chrono::Duration::hours(4))),
        actual_start: Some(Utc::now() - chrono::Duration::hours(2)),
        actual_end: None,
        estimated_hours: payload.estimated_hours.or(Some(8.0)),
        actual_hours: Some(2.0),
        materials: vec![],
        tasks: vec![],
        notes: payload
            .notes
            .or(Some("High priority customer order".to_string())),
        created_at: Utc::now() - chrono::Duration::days(1),
        updated_at: Utc::now(),
    };

    Ok((StatusCode::OK, Json(work_order)))
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
    State(_state): State<S>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, ServiceError>
where
    S: WorkOrdersAppState,
{
    let _ = id; // placeholder until wired to DB
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
    S: WorkOrdersAppState,
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
    S: WorkOrdersAppState,
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
    S: WorkOrdersAppState,
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
    S: WorkOrdersAppState,
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
    S: WorkOrdersAppState,
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
    S: WorkOrdersAppState,
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
    S: WorkOrdersAppState,
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
    S: WorkOrdersAppState,
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
pub async fn assign_work_order<S>(
    State(_state): State<S>,
    Path(id): Path<String>,
    Json(payload): Json<serde_json::Value>,
) -> Result<impl IntoResponse, ServiceError>
where
    S: WorkOrdersAppState,
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
pub async fn update_work_order_status<S>(
    State(_state): State<S>,
    Path(id): Path<String>,
    Json(payload): Json<serde_json::Value>,
) -> Result<impl IntoResponse, ServiceError>
where
    S: WorkOrdersAppState,
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
