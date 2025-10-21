use crate::{
    commands::returns::create_return_command::{InitiateReturnCommand, InitiateReturnResult},
    commands::returns::restock_returned_items_command::RestockReturnedItemsCommand,
    errors::ServiceError,
    models::return_entity,
    ApiResponse, ApiResult, AppState, PaginatedResponse,
};
use axum::{
    extract::{Path, Query, State},
    response::Json,
};
use chrono::{DateTime, NaiveDateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use utoipa::ToSchema;
use uuid::Uuid;
use validator::Validate;

#[derive(Debug, Deserialize, Default, ToSchema)]
pub struct ReturnListQuery {
    /// Page number (1-indexed)
    pub page: Option<u64>,
    /// Page size (max 100)
    pub limit: Option<u64>,
    /// Optional status filter (case-insensitive)
    pub status: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ReturnSummary {
    pub id: Uuid,
    pub order_id: Uuid,
    pub status: String,
    pub reason: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<return_entity::Model> for ReturnSummary {
    fn from(model: return_entity::Model) -> Self {
        Self {
            id: model.id,
            order_id: model.order_id,
            status: model.status,
            reason: model.reason,
            created_at: to_utc(model.created_at),
            updated_at: to_utc(model.updated_at),
        }
    }
}

impl From<InitiateReturnResult> for ReturnSummary {
    fn from(result: InitiateReturnResult) -> Self {
        let created_at = to_utc(result.created_at);
        Self {
            id: result.id,
            order_id: result.order_id,
            status: result.status,
            reason: result.reason,
            created_at,
            updated_at: created_at,
        }
    }
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct CreateReturnRequest {
    pub order_id: Uuid,
    #[validate(length(min = 1, message = "Reason cannot be empty"))]
    pub reason: String,
}

pub async fn list_returns(
    State(state): State<AppState>,
    Query(query): Query<ReturnListQuery>,
) -> ApiResult<PaginatedResponse<ReturnSummary>> {
    let page = query.page.unwrap_or(1).max(1);
    let limit = query.limit.unwrap_or(20).clamp(1, 100);

    let (records, total) = state.return_service().list_returns(page, limit).await?;

    let mut items: Vec<ReturnSummary> = records.into_iter().map(ReturnSummary::from).collect();
    let filtered_total = if let Some(status) = query.status {
        items.retain(|ret| ret.status.eq_ignore_ascii_case(&status));
        items.len() as u64
    } else {
        total
    };

    let total_pages = (filtered_total + limit - 1) / limit;

    Ok(Json(ApiResponse::success(PaginatedResponse {
        items,
        total: filtered_total,
        page,
        limit,
        total_pages,
    })))
}

pub async fn get_return(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> ApiResult<ReturnSummary> {
    match state.return_service().get_return(&id).await? {
        Some(model) => Ok(Json(ApiResponse::success(ReturnSummary::from(model)))),
        None => Err(ServiceError::NotFound(format!("Return {} not found", id))),
    }
}

pub async fn create_return(
    State(state): State<AppState>,
    Json(payload): Json<CreateReturnRequest>,
) -> ApiResult<ReturnSummary> {
    payload
        .validate()
        .map_err(|e| ServiceError::ValidationError(e.to_string()))?;

    let command = InitiateReturnCommand {
        order_id: payload.order_id,
        reason: payload.reason.clone(),
    };

    let created = state.return_service().create_return(command).await?;
    Ok(Json(ApiResponse::success(ReturnSummary::from(created))))
}

pub async fn approve_return(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> ApiResult<ReturnSummary> {
    let updated = state.return_service().approve_return(id).await?;
    Ok(Json(ApiResponse::success(ReturnSummary::from(updated))))
}

pub async fn restock_return(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> ApiResult<serde_json::Value> {
    let command = RestockReturnedItemsCommand { return_id: id };
    state
        .return_service()
        .restock_returned_items(command)
        .await?;
    Ok(Json(ApiResponse::success(json!({
        "return_id": id,
        "status": "restocked"
    }))))
}

fn to_utc(dt: NaiveDateTime) -> DateTime<Utc> {
    DateTime::<Utc>::from_naive_utc_and_offset(dt, Utc)
}
