use crate::{
    commands::warranties::create_warranty_command::CreateWarrantyCommand,
    commands::warranties::{
        approve_warranty_claim_command::ApproveWarrantyClaimCommand,
        claim_warranty_command::ClaimWarrantyCommand,
    },
    entities::warranty,
    errors::ServiceError,
    ApiResponse, ApiResult, AppState, PaginatedResponse,
};
use axum::{
    extract::{Path, Query, State},
    response::Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use utoipa::ToSchema;
use uuid::Uuid;
use validator::Validate;

#[derive(Debug, Deserialize, Default, ToSchema)]
pub struct WarrantyListQuery {
    pub page: Option<u64>,
    pub limit: Option<u64>,
    pub status: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct WarrantySummary {
    pub id: Uuid,
    pub warranty_number: String,
    pub product_id: Uuid,
    pub customer_id: Uuid,
    pub status: String,
    pub start_date: DateTime<Utc>,
    pub end_date: DateTime<Utc>,
    pub description: Option<String>,
    pub terms: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: Option<DateTime<Utc>>,
}

impl From<warranty::Model> for WarrantySummary {
    fn from(model: warranty::Model) -> Self {
        Self {
            id: model.id,
            warranty_number: model.warranty_number,
            product_id: model.product_id,
            customer_id: model.customer_id,
            status: model.status,
            start_date: model.start_date,
            end_date: model.end_date,
            description: model.description,
            terms: model.terms,
            created_at: model.created_at,
            updated_at: model.updated_at,
        }
    }
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct CreateWarrantyRequest {
    pub product_id: Uuid,
    pub customer_id: Uuid,
    #[validate(length(min = 1))]
    pub serial_number: String,
    #[validate(length(min = 1))]
    pub warranty_type: String,
    pub expiration_date: DateTime<Utc>,
    #[validate(length(min = 1))]
    pub terms: String,
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct CreateWarrantyClaimRequest {
    pub warranty_id: Uuid,
    pub customer_id: Uuid,
    #[validate(length(min = 1))]
    pub description: String,
    #[serde(default)]
    pub evidence: Vec<String>,
    pub contact_email: Option<String>,
    pub contact_phone: Option<String>,
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct ApproveWarrantyClaimRequest {
    pub approved_by: Uuid,
    #[validate(length(min = 1))]
    pub resolution: String,
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct ExtendWarrantyRequest {
    pub additional_months: i32,
}

pub async fn list_warranties(
    State(state): State<AppState>,
    Query(query): Query<WarrantyListQuery>,
) -> ApiResult<PaginatedResponse<WarrantySummary>> {
    let page = query.page.unwrap_or(1).max(1);
    let limit = query.limit.unwrap_or(20).clamp(1, 100);

    let (records, total) = state
        .warranty_service()
        .list_warranties(page, limit)
        .await?;

    let mut items: Vec<WarrantySummary> = records.into_iter().map(WarrantySummary::from).collect();
    let filtered_total = if let Some(status) = query.status {
        items.retain(|warranty| warranty.status.eq_ignore_ascii_case(&status));
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

pub async fn get_warranty(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> ApiResult<WarrantySummary> {
    match state.warranty_service().get_warranty(&id).await? {
        Some(warranty) => Ok(Json(ApiResponse::success(WarrantySummary::from(warranty)))),
        None => Err(ServiceError::NotFound(format!("Warranty {} not found", id))),
    }
}

pub async fn create_warranty(
    State(state): State<AppState>,
    Json(payload): Json<CreateWarrantyRequest>,
) -> ApiResult<WarrantySummary> {
    payload
        .validate()
        .map_err(|e| ServiceError::ValidationError(e.to_string()))?;

    let command = CreateWarrantyCommand {
        product_id: payload.product_id,
        customer_id: payload.customer_id,
        serial_number: payload.serial_number.clone(),
        warranty_type: payload.warranty_type.clone(),
        expiration_date: payload.expiration_date,
        terms: payload.terms.clone(),
    };

    let warranty_id = state.warranty_service().create_warranty(command).await?;
    let created = state
        .warranty_service()
        .get_warranty(&warranty_id)
        .await?
        .ok_or_else(|| ServiceError::NotFound(format!("Warranty {} not found", warranty_id)))?;

    Ok(Json(ApiResponse::success(WarrantySummary::from(created))))
}

pub async fn create_warranty_claim(
    State(state): State<AppState>,
    Json(payload): Json<CreateWarrantyClaimRequest>,
) -> ApiResult<serde_json::Value> {
    payload
        .validate()
        .map_err(|e| ServiceError::ValidationError(e.to_string()))?;

    let command = ClaimWarrantyCommand {
        warranty_id: payload.warranty_id,
        customer_id: payload.customer_id,
        description: payload.description.clone(),
        evidence: payload.evidence.clone(),
        contact_email: payload.contact_email.clone(),
        contact_phone: payload.contact_phone.clone(),
    };

    let claim_id = state.warranty_service().claim_warranty(command).await?;
    Ok(Json(ApiResponse::success(json!({
        "claim_id": claim_id
    }))))
}

pub async fn approve_warranty_claim(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(payload): Json<ApproveWarrantyClaimRequest>,
) -> ApiResult<serde_json::Value> {
    payload
        .validate()
        .map_err(|e| ServiceError::ValidationError(e.to_string()))?;

    let command = ApproveWarrantyClaimCommand {
        claim_id: id,
        approved_by: payload.approved_by,
        resolution: payload.resolution.clone(),
        notes: payload.notes.clone(),
    };
    state
        .warranty_service()
        .approve_warranty_claim(command)
        .await?;

    Ok(Json(ApiResponse::success(json!({
        "claim_id": id,
        "status": "approved"
    }))))
}

pub async fn extend_warranty(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(payload): Json<ExtendWarrantyRequest>,
) -> ApiResult<WarrantySummary> {
    payload
        .validate()
        .map_err(|e| ServiceError::ValidationError(e.to_string()))?;

    let updated = state
        .warranty_service()
        .extend_warranty(id, payload.additional_months)
        .await?;
    Ok(Json(ApiResponse::success(WarrantySummary::from(updated))))
}
