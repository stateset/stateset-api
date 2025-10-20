use crate::errors::ServiceError;
use axum::{
    extract::{Json, Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, post, put},
    Router,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;
use uuid::Uuid;

// Generic trait for warranties handler state
pub trait WarrantiesAppState: Clone + Send + Sync + 'static {}
impl<T> WarrantiesAppState for T where T: Clone + Send + Sync + 'static {}

#[derive(Debug, Serialize, Deserialize)]
pub struct Warranty {
    pub id: String,
    pub product_id: String,
    pub customer_id: String,
    pub order_id: Option<String>,
    pub serial_number: Option<String>,
    pub warranty_type: String, // "limited", "extended", "full"
    pub duration_months: i32,
    pub start_date: DateTime<Utc>,
    pub end_date: DateTime<Utc>,
    pub status: String, // "active", "expired", "voided", "claimed"
    pub terms: String,
    pub coverage: Vec<String>,
    pub exclusions: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WarrantyClaim {
    pub id: String,
    pub warranty_id: String,
    pub customer_id: String,
    pub claim_type: String, // "repair", "replacement", "refund"
    pub issue_description: String,
    pub status: String, // "submitted", "under_review", "approved", "rejected", "resolved"
    pub submitted_date: DateTime<Utc>,
    pub resolution_date: Option<DateTime<Utc>>,
    pub resolution_type: Option<String>,
    pub resolution_notes: Option<String>,
    pub repair_cost: Option<f64>,
    pub replacement_product_id: Option<String>,
    pub refund_amount: Option<f64>,
    pub attachments: Vec<ClaimAttachment>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ClaimAttachment {
    pub id: String,
    pub filename: String,
    pub file_type: String,
    pub file_size: i64,
    pub upload_date: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateWarrantyRequest {
    pub product_id: String,
    pub customer_id: String,
    pub order_id: Option<String>,
    pub serial_number: Option<String>,
    pub warranty_type: String,
    pub duration_months: i32,
    pub terms: String,
    pub coverage: Vec<String>,
    pub exclusions: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateWarrantyRequest {
    pub status: Option<String>,
    pub end_date: Option<DateTime<Utc>>,
    pub terms: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateClaimRequest {
    pub warranty_id: String,
    pub claim_type: String,
    pub issue_description: String,
    pub attachments: Option<Vec<String>>, // File IDs or URLs
}

#[derive(Debug, Deserialize)]
pub struct UpdateClaimRequest {
    pub status: Option<String>,
    pub resolution_type: Option<String>,
    pub resolution_notes: Option<String>,
    pub repair_cost: Option<f64>,
    pub replacement_product_id: Option<String>,
    pub refund_amount: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub struct WarrantyFilters {
    pub customer_id: Option<String>,
    pub product_id: Option<String>,
    pub status: Option<String>,
    pub warranty_type: Option<String>,
    pub expiring_soon: Option<bool>, // warranties expiring in next 30 days
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct ClaimFilters {
    pub warranty_id: Option<String>,
    pub customer_id: Option<String>,
    pub status: Option<String>,
    pub claim_type: Option<String>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

/// Create the warranties router
pub fn warranties_router<S>() -> Router<S>
where
    S: WarrantiesAppState,
{
    Router::new()
        .route("/", get(list_warranties::<S>).post(create_warranty::<S>))
        .route(
            "/{id}",
            get(get_warranty::<S>)
                .put(update_warranty::<S>)
                .delete(delete_warranty::<S>),
        )
        .route("/{id}/void", post(void_warranty::<S>))
        .route("/{id}/extend", post(extend_warranty::<S>))
        .route("/claims", get(list_claims::<S>).post(create_claim::<S>))
        .route("/claims/:id", get(get_claim::<S>).put(update_claim::<S>))
        .route("/claims/:id/approve", post(approve_claim::<S>))
        .route("/claims/:id/reject", post(reject_claim::<S>))
        .route("/claims/:id/resolve", post(resolve_claim::<S>))
        .route("/expiring", get(get_expiring_warranties::<S>))
}

/// List warranties with optional filtering
pub async fn list_warranties<S>(
    State(_state): State<S>,
    Query(filters): Query<WarrantyFilters>,
) -> Result<impl IntoResponse, ServiceError>
where
    S: WarrantiesAppState,
{
    // Mock data for now - replace with actual database queries
    let mut warranties = vec![
        Warranty {
            id: "warranty_001".to_string(),
            product_id: "prod_abc".to_string(),
            customer_id: "cust_123".to_string(),
            order_id: Some("order_001".to_string()),
            serial_number: Some("SN123456".to_string()),
            warranty_type: "limited".to_string(),
            duration_months: 12,
            start_date: Utc::now() - chrono::Duration::days(30),
            end_date: Utc::now() + chrono::Duration::days(335),
            status: "active".to_string(),
            terms: "Standard limited warranty covering manufacturing defects".to_string(),
            coverage: vec!["manufacturing_defects".to_string(), "parts".to_string()],
            exclusions: vec!["accidental_damage".to_string(), "wear_and_tear".to_string()],
            created_at: Utc::now() - chrono::Duration::days(30),
            updated_at: Utc::now() - chrono::Duration::days(30),
        },
        Warranty {
            id: "warranty_002".to_string(),
            product_id: "prod_def".to_string(),
            customer_id: "cust_456".to_string(),
            order_id: Some("order_002".to_string()),
            serial_number: Some("SN789012".to_string()),
            warranty_type: "extended".to_string(),
            duration_months: 24,
            start_date: Utc::now() - chrono::Duration::days(10),
            end_date: Utc::now() + chrono::Duration::days(720),
            status: "active".to_string(),
            terms: "Extended warranty with comprehensive coverage".to_string(),
            coverage: vec![
                "manufacturing_defects".to_string(),
                "parts".to_string(),
                "labor".to_string(),
            ],
            exclusions: vec!["accidental_damage".to_string()],
            created_at: Utc::now() - chrono::Duration::days(10),
            updated_at: Utc::now() - chrono::Duration::days(10),
        },
    ];

    // Apply filters
    if let Some(customer_id) = &filters.customer_id {
        warranties.retain(|w| &w.customer_id == customer_id);
    }
    if let Some(product_id) = &filters.product_id {
        warranties.retain(|w| &w.product_id == product_id);
    }
    if let Some(status) = &filters.status {
        warranties.retain(|w| &w.status == status);
    }
    if let Some(warranty_type) = &filters.warranty_type {
        warranties.retain(|w| &w.warranty_type == warranty_type);
    }
    if let Some(true) = filters.expiring_soon {
        let thirty_days_from_now = Utc::now() + chrono::Duration::days(30);
        warranties.retain(|w| w.end_date <= thirty_days_from_now && w.status == "active");
    }

    let response = json!({
        "warranties": warranties,
        "total": warranties.len(),
        "limit": filters.limit.unwrap_or(50),
        "offset": filters.offset.unwrap_or(0)
    });

    Ok((StatusCode::OK, Json(response)))
}

/// Create a new warranty
pub async fn create_warranty<S>(
    State(_state): State<S>,
    Json(payload): Json<CreateWarrantyRequest>,
) -> Result<impl IntoResponse, ServiceError>
where
    S: WarrantiesAppState,
{
    let warranty_id = Uuid::new_v4().to_string();
    let now = Utc::now();
    let end_date = now + chrono::Duration::days(payload.duration_months as i64 * 30);

    let warranty = Warranty {
        id: warranty_id,
        product_id: payload.product_id,
        customer_id: payload.customer_id,
        order_id: payload.order_id,
        serial_number: payload.serial_number,
        warranty_type: payload.warranty_type,
        duration_months: payload.duration_months,
        start_date: now,
        end_date,
        status: "active".to_string(),
        terms: payload.terms,
        coverage: payload.coverage,
        exclusions: payload.exclusions.unwrap_or_default(),
        created_at: now,
        updated_at: now,
    };

    Ok((StatusCode::CREATED, Json(warranty)))
}

/// Get a specific warranty by ID
pub async fn get_warranty<S>(
    State(_state): State<S>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, ServiceError>
where
    S: WarrantiesAppState,
{
    let warranty = Warranty {
        id: id.clone(),
        product_id: "prod_abc".to_string(),
        customer_id: "cust_123".to_string(),
        order_id: Some("order_001".to_string()),
        serial_number: Some("SN123456".to_string()),
        warranty_type: "limited".to_string(),
        duration_months: 12,
        start_date: Utc::now() - chrono::Duration::days(30),
        end_date: Utc::now() + chrono::Duration::days(335),
        status: "active".to_string(),
        terms: "Standard limited warranty covering manufacturing defects".to_string(),
        coverage: vec!["manufacturing_defects".to_string(), "parts".to_string()],
        exclusions: vec!["accidental_damage".to_string(), "wear_and_tear".to_string()],
        created_at: Utc::now() - chrono::Duration::days(30),
        updated_at: Utc::now() - chrono::Duration::days(30),
    };

    Ok((StatusCode::OK, Json(warranty)))
}

/// Update a warranty
pub async fn update_warranty<S>(
    State(_state): State<S>,
    Path(id): Path<String>,
    Json(payload): Json<UpdateWarrantyRequest>,
) -> Result<impl IntoResponse, ServiceError>
where
    S: WarrantiesAppState,
{
    let warranty = Warranty {
        id: id.clone(),
        product_id: "prod_abc".to_string(),
        customer_id: "cust_123".to_string(),
        order_id: Some("order_001".to_string()),
        serial_number: Some("SN123456".to_string()),
        warranty_type: "limited".to_string(),
        duration_months: 12,
        start_date: Utc::now() - chrono::Duration::days(30),
        end_date: payload
            .end_date
            .unwrap_or_else(|| Utc::now() + chrono::Duration::days(335)),
        status: payload.status.unwrap_or_else(|| "active".to_string()),
        terms: payload.terms.unwrap_or_else(|| {
            "Standard limited warranty covering manufacturing defects".to_string()
        }),
        coverage: vec!["manufacturing_defects".to_string(), "parts".to_string()],
        exclusions: vec!["accidental_damage".to_string(), "wear_and_tear".to_string()],
        created_at: Utc::now() - chrono::Duration::days(30),
        updated_at: Utc::now(),
    };

    Ok((StatusCode::OK, Json(warranty)))
}

/// Delete a warranty
pub async fn delete_warranty<S>(
    State(_state): State<S>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, ServiceError>
where
    S: WarrantiesAppState,
{
    let _ = id; // placeholder until wired to DB
    Ok(StatusCode::NO_CONTENT)
}

/// Void a warranty
async fn void_warranty<S>(
    State(_state): State<S>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, ServiceError>
where
    S: WarrantiesAppState,
{
    let response = json!({
        "message": format!("Warranty {} has been voided", id),
        "warranty_id": id,
        "status": "voided",
        "voided_at": Utc::now()
    });

    Ok((StatusCode::OK, Json(response)))
}

/// Extend a warranty
async fn extend_warranty<S>(
    State(_state): State<S>,
    Path(id): Path<String>,
    Json(payload): Json<serde_json::Value>,
) -> Result<impl IntoResponse, ServiceError>
where
    S: WarrantiesAppState,
{
    let additional_months = payload
        .get("additional_months")
        .and_then(|v| v.as_i64())
        .unwrap_or(6) as i32;
    let new_end_date = Utc::now() + chrono::Duration::days(additional_months as i64 * 30);

    let response = json!({
        "message": format!("Warranty {} has been extended by {} months", id, additional_months),
        "warranty_id": id,
        "additional_months": additional_months,
        "new_end_date": new_end_date,
        "extended_at": Utc::now()
    });

    Ok((StatusCode::OK, Json(response)))
}

/// List warranty claims
async fn list_claims<S>(
    State(_state): State<S>,
    Query(filters): Query<ClaimFilters>,
) -> Result<impl IntoResponse, ServiceError>
where
    S: WarrantiesAppState,
{
    let mut claims = vec![
        WarrantyClaim {
            id: "claim_001".to_string(),
            warranty_id: "warranty_001".to_string(),
            customer_id: "cust_123".to_string(),
            claim_type: "repair".to_string(),
            issue_description: "Device stopped working after 3 months of use".to_string(),
            status: "under_review".to_string(),
            submitted_date: Utc::now() - chrono::Duration::days(5),
            resolution_date: None,
            resolution_type: None,
            resolution_notes: None,
            repair_cost: None,
            replacement_product_id: None,
            refund_amount: None,
            attachments: vec![ClaimAttachment {
                id: "att_001".to_string(),
                filename: "device_photo.jpg".to_string(),
                file_type: "image/jpeg".to_string(),
                file_size: 256000,
                upload_date: Utc::now() - chrono::Duration::days(5),
            }],
            created_at: Utc::now() - chrono::Duration::days(5),
            updated_at: Utc::now() - chrono::Duration::days(2),
        },
        WarrantyClaim {
            id: "claim_002".to_string(),
            warranty_id: "warranty_002".to_string(),
            customer_id: "cust_456".to_string(),
            claim_type: "replacement".to_string(),
            issue_description: "Manufacturing defect in display".to_string(),
            status: "approved".to_string(),
            submitted_date: Utc::now() - chrono::Duration::days(10),
            resolution_date: Some(Utc::now() - chrono::Duration::days(2)),
            resolution_type: Some("replacement".to_string()),
            resolution_notes: Some(
                "Approved for replacement under manufacturing defect coverage".to_string(),
            ),
            repair_cost: None,
            replacement_product_id: Some("prod_def_new".to_string()),
            refund_amount: None,
            attachments: vec![],
            created_at: Utc::now() - chrono::Duration::days(10),
            updated_at: Utc::now() - chrono::Duration::days(2),
        },
    ];

    // Apply filters
    if let Some(warranty_id) = &filters.warranty_id {
        claims.retain(|c| &c.warranty_id == warranty_id);
    }
    if let Some(customer_id) = &filters.customer_id {
        claims.retain(|c| &c.customer_id == customer_id);
    }
    if let Some(status) = &filters.status {
        claims.retain(|c| &c.status == status);
    }
    if let Some(claim_type) = &filters.claim_type {
        claims.retain(|c| &c.claim_type == claim_type);
    }

    let response = json!({
        "claims": claims,
        "total": claims.len(),
        "limit": filters.limit.unwrap_or(50),
        "offset": filters.offset.unwrap_or(0)
    });

    Ok((StatusCode::OK, Json(response)))
}

/// Create a new warranty claim
async fn create_claim<S>(
    State(_state): State<S>,
    Json(payload): Json<CreateClaimRequest>,
) -> Result<impl IntoResponse, ServiceError>
where
    S: WarrantiesAppState,
{
    let claim_id = Uuid::new_v4().to_string();
    let now = Utc::now();

    let claim = WarrantyClaim {
        id: claim_id,
        warranty_id: payload.warranty_id,
        customer_id: "cust_123".to_string(), // Mock - get from warranty
        claim_type: payload.claim_type,
        issue_description: payload.issue_description,
        status: "submitted".to_string(),
        submitted_date: now,
        resolution_date: None,
        resolution_type: None,
        resolution_notes: None,
        repair_cost: None,
        replacement_product_id: None,
        refund_amount: None,
        attachments: vec![], // Mock - handle file uploads separately
        created_at: now,
        updated_at: now,
    };

    Ok((StatusCode::CREATED, Json(claim)))
}

/// Get a specific warranty claim
async fn get_claim<S>(
    State(_state): State<S>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, ServiceError>
where
    S: WarrantiesAppState,
{
    let claim = WarrantyClaim {
        id: id.clone(),
        warranty_id: "warranty_001".to_string(),
        customer_id: "cust_123".to_string(),
        claim_type: "repair".to_string(),
        issue_description: "Device stopped working after 3 months of use".to_string(),
        status: "under_review".to_string(),
        submitted_date: Utc::now() - chrono::Duration::days(5),
        resolution_date: None,
        resolution_type: None,
        resolution_notes: None,
        repair_cost: None,
        replacement_product_id: None,
        refund_amount: None,
        attachments: vec![],
        created_at: Utc::now() - chrono::Duration::days(5),
        updated_at: Utc::now() - chrono::Duration::days(2),
    };

    Ok((StatusCode::OK, Json(claim)))
}

/// Update a warranty claim
async fn update_claim<S>(
    State(_state): State<S>,
    Path(id): Path<String>,
    Json(payload): Json<UpdateClaimRequest>,
) -> Result<impl IntoResponse, ServiceError>
where
    S: WarrantiesAppState,
{
    let claim = WarrantyClaim {
        id: id.clone(),
        warranty_id: "warranty_001".to_string(),
        customer_id: "cust_123".to_string(),
        claim_type: "repair".to_string(),
        issue_description: "Device stopped working after 3 months of use".to_string(),
        status: payload.status.unwrap_or_else(|| "under_review".to_string()),
        submitted_date: Utc::now() - chrono::Duration::days(5),
        resolution_date: if payload.resolution_type.is_some() {
            Some(Utc::now())
        } else {
            None
        },
        resolution_type: payload.resolution_type,
        resolution_notes: payload.resolution_notes,
        repair_cost: payload.repair_cost,
        replacement_product_id: payload.replacement_product_id,
        refund_amount: payload.refund_amount,
        attachments: vec![],
        created_at: Utc::now() - chrono::Duration::days(5),
        updated_at: Utc::now(),
    };

    Ok((StatusCode::OK, Json(claim)))
}

/// Approve a warranty claim
async fn approve_claim<S>(
    State(_state): State<S>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, ServiceError>
where
    S: WarrantiesAppState,
{
    let response = json!({
        "message": format!("Warranty claim {} has been approved", id),
        "claim_id": id,
        "status": "approved",
        "approved_at": Utc::now()
    });

    Ok((StatusCode::OK, Json(response)))
}

/// Reject a warranty claim
async fn reject_claim<S>(
    State(_state): State<S>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, ServiceError>
where
    S: WarrantiesAppState,
{
    let response = json!({
        "message": format!("Warranty claim {} has been rejected", id),
        "claim_id": id,
        "status": "rejected",
        "rejected_at": Utc::now()
    });

    Ok((StatusCode::OK, Json(response)))
}

/// Resolve a warranty claim
async fn resolve_claim<S>(
    State(_state): State<S>,
    Path(id): Path<String>,
    Json(payload): Json<serde_json::Value>,
) -> Result<impl IntoResponse, ServiceError>
where
    S: WarrantiesAppState,
{
    let resolution_type = payload
        .get("resolution_type")
        .and_then(|v| v.as_str())
        .unwrap_or("repair");

    let response = json!({
        "message": format!("Warranty claim {} has been resolved", id),
        "claim_id": id,
        "status": "resolved",
        "resolution_type": resolution_type,
        "resolved_at": Utc::now()
    });

    Ok((StatusCode::OK, Json(response)))
}

/// Get warranties expiring soon
async fn get_expiring_warranties<S>(
    State(_state): State<S>,
    Query(filters): Query<serde_json::Value>,
) -> Result<impl IntoResponse, ServiceError>
where
    S: WarrantiesAppState,
{
    let days_ahead = filters.get("days").and_then(|v| v.as_i64()).unwrap_or(30);
    let expiring_date = Utc::now() + chrono::Duration::days(days_ahead);

    // Mock expiring warranties
    let expiring_warranties = vec![json!({
        "id": "warranty_003",
        "product_id": "prod_xyz",
        "customer_id": "cust_789",
        "end_date": expiring_date - chrono::Duration::days(15),
        "days_until_expiry": 15
    })];

    let response = json!({
        "expiring_warranties": expiring_warranties,
        "total": expiring_warranties.len(),
        "days_ahead": days_ahead
    });

    Ok((StatusCode::OK, Json(response)))
}

/// Create a warranty claim
pub async fn create_warranty_claim<S>(
    State(_state): State<S>,
    Json(payload): Json<serde_json::Value>,
) -> Result<impl IntoResponse, ServiceError>
where
    S: WarrantiesAppState,
{
    let warranty_id = payload
        .get("warranty_id")
        .and_then(|w| w.as_str())
        .unwrap_or("warranty-123");
    let claim_reason = payload
        .get("reason")
        .and_then(|r| r.as_str())
        .unwrap_or("Defective product");

    let claim = json!({
        "id": Uuid::new_v4().to_string(),
        "warranty_id": warranty_id,
        "reason": claim_reason,
        "status": "submitted",
        "claim_date": Utc::now(),
        "description": payload.get("description").unwrap_or(&json!("Warranty claim submitted")),
        "attachments": payload.get("attachments").unwrap_or(&json!([]))
    });

    Ok((StatusCode::CREATED, Json(claim)))
}
