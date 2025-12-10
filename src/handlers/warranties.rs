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
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;
use validator::Validate;

#[derive(Debug, Deserialize, Default, ToSchema, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct WarrantyListQuery {
    pub page: Option<u64>,
    pub limit: Option<u64>,
    pub status: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(example = json!({
    "id": "aa0e8400-e29b-41d4-a716-446655440000",
    "warranty_number": "WRN-2024-001234",
    "product_id": "550e8400-e29b-41d4-a716-446655440000",
    "customer_id": "123e4567-e89b-12d3-a456-426614174000",
    "status": "active",
    "start_date": "2024-12-09T00:00:00Z",
    "end_date": "2026-12-09T00:00:00Z",
    "description": "2-year manufacturer warranty covering defects in materials and workmanship",
    "terms": "Covers manufacturing defects only. Does not cover damage from misuse, accidents, or unauthorized modifications.",
    "created_at": "2024-12-09T10:30:00Z",
    "updated_at": "2024-12-09T10:30:00Z"
}))]
pub struct WarrantySummary {
    /// Warranty UUID
    #[schema(example = "aa0e8400-e29b-41d4-a716-446655440000")]
    pub id: Uuid,
    /// Human-readable warranty number
    #[schema(example = "WRN-2024-001234")]
    pub warranty_number: String,
    /// Product UUID covered by warranty
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub product_id: Uuid,
    /// Customer UUID who owns the warranty
    #[schema(example = "123e4567-e89b-12d3-a456-426614174000")]
    pub customer_id: Uuid,
    /// Warranty status (active, expired, claimed, void)
    #[schema(example = "active")]
    pub status: String,
    /// Warranty start date
    pub start_date: DateTime<Utc>,
    /// Warranty expiration date
    pub end_date: DateTime<Utc>,
    /// Warranty description
    #[schema(example = "2-year manufacturer warranty covering defects")]
    pub description: Option<String>,
    /// Warranty terms and conditions
    #[schema(example = "Covers manufacturing defects only")]
    pub terms: Option<String>,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
    /// Last update timestamp
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
#[schema(example = json!({
    "product_id": "550e8400-e29b-41d4-a716-446655440000",
    "customer_id": "123e4567-e89b-12d3-a456-426614174000",
    "serial_number": "SN-WBH-2024-001234",
    "warranty_type": "manufacturer",
    "expiration_date": "2026-12-09T00:00:00Z",
    "terms": "Covers manufacturing defects only. Does not cover damage from misuse."
}))]
pub struct CreateWarrantyRequest {
    /// Product UUID to warranty
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub product_id: Uuid,
    /// Customer UUID
    #[schema(example = "123e4567-e89b-12d3-a456-426614174000")]
    pub customer_id: Uuid,
    /// Product serial number
    #[validate(length(min = 1))]
    #[schema(example = "SN-WBH-2024-001234")]
    pub serial_number: String,
    /// Warranty type (manufacturer, extended, limited)
    #[validate(length(min = 1))]
    #[schema(example = "manufacturer")]
    pub warranty_type: String,
    /// Warranty expiration date
    pub expiration_date: DateTime<Utc>,
    /// Warranty terms and conditions
    #[validate(length(min = 1))]
    #[schema(example = "Covers manufacturing defects only")]
    pub terms: String,
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
#[schema(example = json!({
    "warranty_id": "aa0e8400-e29b-41d4-a716-446655440000",
    "customer_id": "123e4567-e89b-12d3-a456-426614174000",
    "description": "Product stopped working after 3 months of normal use. Display shows error code E01.",
    "evidence": ["https://storage.example.com/claims/photo1.jpg", "https://storage.example.com/claims/receipt.pdf"],
    "contact_email": "customer@example.com",
    "contact_phone": "+1-555-123-4567"
}))]
pub struct CreateWarrantyClaimRequest {
    /// Warranty UUID to claim
    #[schema(example = "aa0e8400-e29b-41d4-a716-446655440000")]
    pub warranty_id: Uuid,
    /// Customer UUID filing the claim
    #[schema(example = "123e4567-e89b-12d3-a456-426614174000")]
    pub customer_id: Uuid,
    /// Description of the issue
    #[validate(length(min = 1))]
    #[schema(example = "Product stopped working after 3 months of normal use")]
    pub description: String,
    /// URLs to supporting evidence (photos, receipts, etc.)
    #[serde(default)]
    pub evidence: Vec<String>,
    /// Contact email for claim updates
    #[schema(example = "customer@example.com")]
    pub contact_email: Option<String>,
    /// Contact phone for claim updates
    #[schema(example = "+1-555-123-4567")]
    pub contact_phone: Option<String>,
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
#[schema(example = json!({
    "approved_by": "bb0e8400-e29b-41d4-a716-446655440000",
    "resolution": "replacement",
    "notes": "Approved replacement unit. Original unit will be collected upon delivery."
}))]
pub struct ApproveWarrantyClaimRequest {
    /// Admin/agent UUID approving the claim
    #[schema(example = "bb0e8400-e29b-41d4-a716-446655440000")]
    pub approved_by: Uuid,
    /// Resolution type (replacement, repair, refund)
    #[validate(length(min = 1))]
    #[schema(example = "replacement")]
    pub resolution: String,
    /// Additional notes for the claim
    #[schema(example = "Approved replacement unit")]
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
#[schema(example = json!({
    "additional_months": 12
}))]
pub struct ExtendWarrantyRequest {
    /// Number of months to extend the warranty
    #[schema(example = 12)]
    pub additional_months: i32,
}

#[utoipa::path(
    get,
    path = "/api/v1/warranties",
    params(WarrantyListQuery),
    responses(
        (status = 200, description = "Warranties listed", body = ApiResponse<PaginatedResponse<WarrantySummary>>),
        (status = 401, description = "Unauthorized", body = crate::errors::ErrorResponse),
        (status = 403, description = "Forbidden", body = crate::errors::ErrorResponse)
    ),
    tag = "warranties"
)]
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

#[utoipa::path(
    get,
    path = "/api/v1/warranties/:id",
    params(
        ("id" = Uuid, Path, description = "Warranty ID")
    ),
    responses(
        (status = 200, description = "Warranty fetched", body = ApiResponse<WarrantySummary>),
        (status = 404, description = "Warranty not found", body = crate::errors::ErrorResponse)
    ),
    tag = "warranties"
)]
pub async fn get_warranty(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> ApiResult<WarrantySummary> {
    match state.warranty_service().get_warranty(&id).await? {
        Some(warranty) => Ok(Json(ApiResponse::success(WarrantySummary::from(warranty)))),
        None => Err(ServiceError::NotFound(format!("Warranty {} not found", id))),
    }
}

#[utoipa::path(
    post,
    path = "/api/v1/warranties",
    request_body = CreateWarrantyRequest,
    responses(
        (status = 201, description = "Warranty created", body = ApiResponse<WarrantySummary>),
        (status = 400, description = "Invalid request", body = crate::errors::ErrorResponse)
    ),
    tag = "warranties"
)]
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

#[utoipa::path(
    post,
    path = "/api/v1/warranties/claims",
    request_body = CreateWarrantyClaimRequest,
    responses(
        (status = 201, description = "Warranty claim created", body = ApiResponse<serde_json::Value>),
        (status = 400, description = "Invalid request", body = crate::errors::ErrorResponse),
        (status = 404, description = "Warranty not found", body = crate::errors::ErrorResponse)
    ),
    tag = "warranties"
)]
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

#[utoipa::path(
    post,
    path = "/api/v1/warranties/claims/:id/approve",
    request_body = ApproveWarrantyClaimRequest,
    params(
        ("id" = Uuid, Path, description = "Warranty claim ID")
    ),
    responses(
        (status = 200, description = "Warranty claim approved", body = ApiResponse<serde_json::Value>),
        (status = 400, description = "Invalid request", body = crate::errors::ErrorResponse),
        (status = 404, description = "Warranty claim not found", body = crate::errors::ErrorResponse)
    ),
    tag = "warranties"
)]
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

#[utoipa::path(
    post,
    path = "/api/v1/warranties/:id/extend",
    request_body = ExtendWarrantyRequest,
    params(
        ("id" = Uuid, Path, description = "Warranty ID")
    ),
    responses(
        (status = 200, description = "Warranty extended", body = ApiResponse<WarrantySummary>),
        (status = 400, description = "Invalid request", body = crate::errors::ErrorResponse),
        (status = 404, description = "Warranty not found", body = crate::errors::ErrorResponse)
    ),
    tag = "warranties"
)]
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
