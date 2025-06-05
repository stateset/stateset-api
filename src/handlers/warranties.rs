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
    services::warranties::WarrantyService,
    commands::warranties::{
        create_warranty_command::CreateWarrantyCommand,
        claim_warranty_command::ClaimWarrantyCommand,
        approve_warranty_claim_command::ApproveWarrantyClaimCommand,
        reject_warranty_claim_command::RejectWarrantyClaimCommand,
    },
    main::AppState,
};
use serde::{Serialize, Deserialize};
use validator::Validate;
use tracing::info;
use chrono::{NaiveDateTime, NaiveDate};
use super::common::{validate_input, map_service_error, success_response, created_response, no_content_response, PaginationParams};

// Request and response DTOs

#[derive(Debug, Deserialize, Validate)]
pub struct CreateWarrantyRequest {
    pub product_id: Uuid,
    pub customer_id: Uuid,
    #[validate(length(min = 1, message = "Serial number cannot be empty"))]
    pub serial_number: String,
    #[validate(length(min = 1, message = "Warranty type cannot be empty"))]
    pub warranty_type: String,
    pub expiration_date: String,
    pub terms: String,
}

#[derive(Debug, Deserialize, Validate)]
pub struct ClaimWarrantyRequest {
    pub customer_id: Uuid,
    #[validate(length(min = 1, message = "Description cannot be empty"))]
    pub description: String,
    pub evidence: Option<Vec<String>>,
    pub contact_email: Option<String>,
    pub contact_phone: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct ApproveWarrantyClaimRequest {
    pub approved_by: Uuid,
    pub resolution: String,
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct RejectWarrantyClaimRequest {
    pub rejected_by: Uuid,
    #[validate(length(min = 1, message = "Reason cannot be empty"))]
    pub reason: String,
    pub notes: Option<String>,
}

// Handler functions

/// Create a new warranty
async fn create_warranty(
    State(state): State<Arc<AppState>>,
    AuthenticatedUser(user): AuthenticatedUser,
    Json(payload): Json<CreateWarrantyRequest>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    validate_input(&payload)?;
    
    // Parse the expiration date
    let expiration_date = NaiveDate::parse_from_str(&payload.expiration_date, "%Y-%m-%d")
        .map_err(|e| ApiError::BadRequest(format!("Invalid date format: {}", e)))?
        .and_hms_opt(23, 59, 59)
        .unwrap();
    
    let command = CreateWarrantyCommand {
        product_id: payload.product_id,
        customer_id: payload.customer_id,
        serial_number: payload.serial_number,
        warranty_type: payload.warranty_type,
        expiration_date,
        terms: payload.terms,
    };
    
    let warranty_id = state.services.warranties
        .create_warranty(command)
        .await
        .map_err(map_service_error)?;
    
    info!("Warranty created: {}", warranty_id);
    
    created_response(serde_json::json!({
        "id": warranty_id,
        "message": "Warranty created successfully"
    }))
}

/// Get a warranty by ID
async fn get_warranty(
    State(state): State<Arc<AppState>>,
    Path(warranty_id): Path<Uuid>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    let warranty = state.services.warranties
        .get_warranty(&warranty_id)
        .await
        .map_err(map_service_error)?
        .ok_or_else(|| ApiError::NotFound(format!("Warranty with ID {} not found", warranty_id)))?;
    
    success_response(warranty)
}

/// Claim a warranty
async fn claim_warranty(
    State(state): State<Arc<AppState>>,
    Path(warranty_id): Path<Uuid>,
    Json(payload): Json<ClaimWarrantyRequest>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    validate_input(&payload)?;
    
    let command = ClaimWarrantyCommand {
        warranty_id,
        customer_id: payload.customer_id,
        description: payload.description,
        evidence: payload.evidence.unwrap_or_default(),
        contact_email: payload.contact_email,
        contact_phone: payload.contact_phone,
    };
    
    let claim_id = state.services.warranties
        .claim_warranty(command)
        .await
        .map_err(map_service_error)?;
    
    info!("Warranty claim created: {} for warranty: {}", claim_id, warranty_id);
    
    created_response(serde_json::json!({
        "claim_id": claim_id,
        "message": "Warranty claim created successfully"
    }))
}

/// Approve a warranty claim
async fn approve_warranty_claim(
    State(state): State<Arc<AppState>>,
    Path(claim_id): Path<Uuid>,
    Json(payload): Json<ApproveWarrantyClaimRequest>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    validate_input(&payload)?;
    
    let command = ApproveWarrantyClaimCommand {
        claim_id,
        approved_by: payload.approved_by,
        resolution: payload.resolution,
        notes: payload.notes,
    };
    
    state.services.warranties
        .approve_warranty_claim(command)
        .await
        .map_err(map_service_error)?;
    
    info!("Warranty claim approved: {}", claim_id);
    
    success_response(serde_json::json!({
        "message": "Warranty claim approved successfully"
    }))
}

/// Reject a warranty claim
async fn reject_warranty_claim(
    State(state): State<Arc<AppState>>,
    Path(claim_id): Path<Uuid>,
    Json(payload): Json<RejectWarrantyClaimRequest>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    validate_input(&payload)?;
    
    let command = RejectWarrantyClaimCommand {
        claim_id,
        rejected_by: payload.rejected_by,
        reason: payload.reason,
        notes: payload.notes,
    };
    
    state.services.warranties
        .reject_warranty_claim(command)
        .await
        .map_err(map_service_error)?;
    
    info!("Warranty claim rejected: {}", claim_id);
    
    success_response(serde_json::json!({
        "message": "Warranty claim rejected successfully"
    }))
}

/// Get warranties for a product
async fn get_warranties_for_product(
    State(state): State<Arc<AppState>>,
    Path(product_id): Path<Uuid>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    let warranties = state.services.warranties
        .get_warranties_for_product(&product_id)
        .await
        .map_err(map_service_error)?;
    
    success_response(warranties)
}

/// Get active warranties for a customer
async fn get_active_warranties_for_customer(
    State(state): State<Arc<AppState>>,
    Path(customer_id): Path<Uuid>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    let warranties = state.services.warranties
        .get_active_warranties_for_customer(&customer_id)
        .await
        .map_err(map_service_error)?;
    
    success_response(warranties)
}

/// Check if a product is under warranty
async fn check_warranty_status(
    State(state): State<Arc<AppState>>,
    Path((product_id, serial_number)): Path<(Uuid, String)>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    let is_under_warranty = state.services.warranties
        .is_under_warranty(&product_id, &serial_number)
        .await
        .map_err(map_service_error)?;
    
    success_response(serde_json::json!({
        "product_id": product_id,
        "serial_number": serial_number,
        "under_warranty": is_under_warranty
    }))
}

/// Creates the router for warranty endpoints
pub fn warranties_routes() -> Router {
    Router::new()
        .route("/", post(create_warranty))
        .route("/:id", get(get_warranty))
        .route("/:id/claim", post(claim_warranty))
        .route("/claims/:id/approve", post(approve_warranty_claim))
        .route("/claims/:id/reject", post(reject_warranty_claim))
        .route("/product/:product_id", get(get_warranties_for_product))
        .route("/customer/:customer_id", get(get_active_warranties_for_customer))
        .route("/check/:product_id/:serial_number", get(check_warranty_status))
}
