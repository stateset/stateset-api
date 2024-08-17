use axum::{
    routing::{post, get, put},
    extract::{Path, State, Query, Json},
    response::IntoResponse,
    Router,
};
use crate::services::returns::ReturnService;
use crate::models::{NewReturn, Return, ReturnStatus, ReturnSearchParams};
use crate::errors::{ServiceError, ReturnError};
use crate::auth::AuthenticatedUser;
use crate::utils::pagination::PaginationParams;
use validator::Validate;
use uuid::Uuid;
use std::sync::Arc;

use crate::commands::returns::{
    ApproveReturnCommand,
    RejectReturnCommand,
    CancelReturnCommand,
    CompleteReturnCommand,
    CloseReturnCommand,
    ReOpenReturnCommand,
    RestockReturnedItemsCommand,
    RefundReturnCommand,
    ProcessReturnCommand,
    DeleteReturnCommand,
};

async fn create_return(
    State(return_service): State<Arc<ReturnService>>,
    AuthenticatedUser(user): AuthenticatedUser,
    Json(return_info): Json<NewReturn>,
) -> Result<impl IntoResponse, ServiceError> {
    let command = CreateReturnCommand {
        return_info,
        user_id: user.user_id,
    };

    let created_return = command.execute(return_service).await?;
    Ok((axum::http::StatusCode::CREATED, Json(created_return)))
}

async fn approve_return(
    State(return_service): State<Arc<ReturnService>>,
    Path(return_id): Path<i32>,
) -> Result<impl IntoResponse, ServiceError> {
    let command = ApproveReturnCommand { return_id };

    let approved_return = command.execute(return_service).await?;
    Ok(Json(approved_return))
}

async fn reject_return(
    State(return_service): State<Arc<ReturnService>>,
    Path(return_id): Path<i32>,
    Json(reject_info): Json<RejectReturnCommand>,
) -> Result<impl IntoResponse, ServiceError> {
    let command = RejectReturnCommand {
        return_id,
        reason: reject_info.reason,
    };

    let rejected_return = command.execute(return_service).await?;
    Ok(Json(rejected_return))
}

async fn cancel_return(
    State(return_service): State<Arc<ReturnService>>,
    Path(return_id): Path<i32>,
    Json(cancel_info): Json<CancelReturnCommand>,
) -> Result<impl IntoResponse, ServiceError> {
    let command = CancelReturnCommand {
        return_id,
        reason: cancel_info.reason,
    };

    let result = command.execute(return_service).await?;
    Ok(Json(result))
}

async fn close_return(
    State(return_service): State<Arc<ReturnService>>,
    Path(return_id): Path<i32>,
) -> Result<impl IntoResponse, ServiceError> {
    let command = CloseReturnCommand { return_id };

    let closed_return = command.execute(return_service).await?;
    Ok(Json(closed_return))
}

async fn reopen_return(
    State(return_service): State<Arc<ReturnService>>,
    Path(return_id): Path<i32>,
) -> Result<impl IntoResponse, ServiceError> {
    let command = ReopenReturnCommand { return_id };

    let reopened_return = command.execute(return_service).await?;
    Ok(Json(reopened_return))
}

async fn delete_return(
    State(return_service): State<Arc<ReturnService>>,
    Path(id): Path<Uuid>,
    AuthenticatedUser(user): AuthenticatedUser,
) -> Result<impl IntoResponse, ServiceError> {

    let command = DeleteReturnCommand { return_id: id };

    let deleted_return = command.execute(return_service).await?;
    Ok(Json(deleted_return))
}

async fn update_return(
    State(return_service): State<Arc<ReturnService>>,
    Path(id): Path<Uuid>,
    AuthenticatedUser(user): AuthenticatedUser,
    Json(return_info): Json<Return>,
) -> Result<impl IntoResponse, ServiceError> {
    return_info.validate().map_err(|e| ServiceError::BadRequest(e.to_string()))?;
    let updated_return = return_service.update_return(id, return_info, user.user_id)
        .await
        .map_err(|e| ServiceError::from(ReturnError::from(e)))?;
    Ok(Json(updated_return))
}

async fn list_returns(
    State(return_service): State<Arc<ReturnService>>,
    AuthenticatedUser(user): AuthenticatedUser,
    Query(query): Query<PaginationParams>,
) -> Result<impl IntoResponse, ServiceError> {
    let (returns, total) = return_service.list_returns(user.user_id, query)
        .await
        .map_err(|e| ServiceError::from(ReturnError::from(e)))?;
    Ok(Json(json!({
        "returns": returns,
        "total": total,
        "page": query.page,
        "per_page": query.per_page
    })))
}

async fn search_returns(
    State(return_service): State<Arc<ReturnService>>,
    AuthenticatedUser(user): AuthenticatedUser,
    Query(query): Query<ReturnSearchParams>,
    Query(pagination): Query<PaginationParams>,
) -> Result<impl IntoResponse, ServiceError> {
    let (returns, total) = return_service.search_returns(user.user_id, query, pagination)
        .await
        .map_err(|e| ServiceError::from(ReturnError::from(e)))?;
    Ok(Json(json!({
        "returns": returns,
        "total": total,
        "query": query,
        "page": pagination.page,
        "per_page": pagination.per_page
    })))
}

pub fn returns_routes() -> Router {
    Router::new()
        .route("/", post(create_return))
        .route("/:id", get(get_return).put(update_return))
        .route("/", get(list_returns))
        .route("/search", get(search_returns))
        .route("/:id/approve", post(approve_return))
        .route("/:id/reject", post(reject_return))
        .route("/:id/cancel", post(cancel_return))
        .route("/:id/delete", delete(delete_return))
        .route("/:id/restock", post(restock_return))
        .route("/:id/refund", post(refund_return))
        .route("/:id/complete", post(complete_return))
        .route("/:id/close", post(close_return))
        .route("/:id/reopen", post(reopen_return))
        .route("/:id/process", post(process_return))
}