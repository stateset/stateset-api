use axum::{
    extract::{Path, Query, State},
    routing::{get, post, put, delete},
    Json, Router,
};
use std::sync::Arc;
use crate::db::DbPool;
use crate::models::work_order::{NewWorkOrder, WorkOrder, WorkOrderSearchParams, WorkOrderStatus};
use crate::errors::ServiceError;
use crate::services::work_orders::{
    create_work_order, get_work_order, update_work_order, delete_work_order,
    list_work_orders, search_work_orders, assign_work_order, complete_work_order
};
use crate::auth::AuthenticatedUser;
use validator::Validate;

async fn create_work_order(
    State(db_pool): State<Arc<DbPool>>,
    Json(work_order_info): Json<NewWorkOrder>,
    user: AuthenticatedUser,
) -> Result<Json<WorkOrder>, ServiceError> {
    work_order_info.validate()?;

    let command = CreateWorkOrderCommand {
        work_order_info,
        user_id: user.user_id,
    };

    let created_work_order = command.execute(db_pool).await?;
    Ok(Json(created_work_order))
}

async fn get_work_order(
    State(db_pool): State<Arc<DbPool>>,
    Path(id): Path<i32>,
) -> Result<Json<WorkOrder>, ServiceError> {
    let command = GetWorkOrderCommand { id };
    let work_order = command.execute(db_pool).await?;
    Ok(Json(work_order))
}

async fn update_work_order(
    State(db_pool): State<Arc<DbPool>>,
    Path(id): Path<i32>,
    Json(work_order_info): Json<WorkOrder>,
) -> Result<Json<WorkOrder>, ServiceError> {
    work_order_info.validate()?;

    let command = UpdateWorkOrderCommand {
        id,
        work_order_info,
    };

    let updated_work_order = command.execute(db_pool).await?;
    Ok(Json(updated_work_order))
}

async fn delete_work_order(
    State(db_pool): State<Arc<DbPool>>,
    Path(id): Path<i32>,
) -> Result<(), ServiceError> {
    let command = DeleteWorkOrderCommand { id };
    command.execute(db_pool).await?;
    Ok(())
}

async fn list_work_orders(
    State(pool): State<Arc<DbPool>>,
    Query(query): Query<PaginationParams>,
    _user: AuthenticatedUser,
) -> Result<Json<Vec<WorkOrder>>, ServiceError> {
    let work_orders = list_work_orders(&pool, query).await?;
    Ok(Json(work_orders))
}

async fn search_work_orders(
    State(pool): State<Arc<DbPool>>,
    Query(query): Query<WorkOrderSearchParams>,
    _user: AuthenticatedUser,
) -> Result<Json<Vec<WorkOrder>>, ServiceError> {
    let work_orders = search_work_orders(&pool, query).await?;
    Ok(Json(work_orders))
}

async fn assign_work_order(
    State(db_pool): State<Arc<DbPool>>,
    Path(work_order_id): Path<i32>,
    Json(user_id): Json<i32>,
    _user: AuthenticatedUser,
) -> Result<Json<WorkOrder>, ServiceError> {
    let command = AssignWorkOrderCommand {
        work_order_id,
        user_id,
    };

    let result = command.execute(db_pool).await?;
    Ok(Json(result))
}

async fn unassign_work_order(
    State(db_pool): State<Arc<DbPool>>,
    Path(work_order_id): Path<i32>,
    _user: AuthenticatedUser,
) -> Result<Json<WorkOrder>, ServiceError> {
    let command = UnassignWorkOrderCommand { work_order_id };

    let result = command.execute(db_pool).await?;
    Ok(Json(result))
}

async fn complete_work_order(
    State(db_pool): State<Arc<DbPool>>,
    Path(work_order_id): Path<i32>,
    Json(actual_duration): Json<i32>,
    user: AuthenticatedUser,
) -> Result<Json<WorkOrder>, ServiceError> {
    let command = CompleteWorkOrderCommand {
        work_order_id,
        actual_duration,
        completed_by: user.user_id,
    };

    let result = command.execute(db_pool).await?;
    Ok(Json(result))
}

async fn issue_work_order(
    State(db_pool): State<Arc<DbPool>>,
    Path(work_order_id): Path<i32>,
    _user: AuthenticatedUser,
) -> Result<Json<WorkOrder>, ServiceError> {
    let command = IssueWorkOrderCommand { work_order_id };

    let result = command.execute(&db_pool).await?;
    Ok(Json(result))
}

async fn pick_work_order(
    State(db_pool): State<Arc<DbPool>>,
    Path(work_order_id): Path<i32>,
    _user: AuthenticatedUser,
) -> Result<Json<WorkOrder>, ServiceError> {
    let command = PickWorkOrderCommand { work_order_id };

    let result = command.execute(&db_pool).await?;
    Ok(Json(result))
}

async fn yield_work_order(
    State(db_pool): State<Arc<DbPool>>,
    Path(work_order_id): Path<i32>,
    _user: AuthenticatedUser,
) -> Result<Json<WorkOrder>, ServiceError> {
    let command = YieldWorkOrderCommand { work_order_id };

    let result = command.execute(&db_pool).await?;
    Ok(Json(result))
}

pub fn work_order_routes() -> Router<Arc<DbPool>> {
    Router::new()
        .route("/", post(create_work_order))
        .route("/:id", get(get_work_order))
        .route("/:id", put(update_work_order))
        .route("/:id", delete(delete_work_order))
        .route("/", get(list_work_orders))
        .route("/search", get(search_work_orders))
        .route("/:id/assign", post(assign_work_order))
        .route("/:id/unassign", post(unassign_work_order))
        .route("/:id/complete", post(complete_work_order))
        .route("/:id/issue", post(issue_work_order))
        .route("/:id/pick", post(pick_work_order))
        .route("/:id/yield", post(yield_work_order))
}