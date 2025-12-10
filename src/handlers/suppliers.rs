use super::common::{
    created_response, map_service_error, no_content_response, success_response, validate_input,
    PaginationParams,
};
use crate::{
    auth::AuthenticatedUser,
    commands::suppliers::{
        create_supplier_command::CreateSupplierCommand,
        delete_supplier_command::DeleteSupplierCommand,
        update_supplier_command::UpdateSupplierCommand,
    },
    errors::{ApiError, ServiceError},
    handlers::AppState,
    services::suppliers::SupplierService,
};
use axum::{
    extract::{Json, Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, post, put},
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::info;
use uuid::Uuid;
use validator::Validate;

// Request and response DTOs

/* error[E0308]: mismatched types
   --> src/handlers/suppliers.rs:231:5
    |
230 |   pub fn supplier_routes() -> Router {
    |                               ------ expected `axum::Router` because of return type
231 | /     Router::new()
232 | |         .route("/", post(create_supplier))
233 | |         .route("/", get(list_suppliers))
234 | |         .route("/:id", get(get_supplier))
...   |
238 | |         .route("/rating/:min_rating", get(get_suppliers_by_min_rating))
239 | |         .route("/name", get(get_supplier_by_name))
    | |__________________________________________________^ expected `Router`, found `Router<RcOrArc<AppState>>`
    |
    = note: expected struct `axum::Router<()>`
               found struct `axum::Router<RcOrArc<AppState>>`
 */

#[derive(Debug, Deserialize, Validate)]
pub struct CreateSupplierRequest {
    
    pub name: String,
    
    pub contact_name: String,
    
    pub email: String,
    
    pub phone: String,
    pub address: String,
    pub category: String,
    pub payment_terms: String,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateSupplierRequest {
    pub name: Option<String>,
    pub contact_name: Option<String>,
    
    pub email: Option<String>,
    pub phone: Option<String>,
    pub address: Option<String>,
    pub category: Option<String>,
    pub payment_terms: Option<String>,
    pub rating: Option<f32>,
}

// Handler functions

/// Create a new supplier
async fn create_supplier(
    State(state): State<Arc<AppState>>,
    user: AuthenticatedUser,
    Json(payload): Json<CreateSupplierRequest>,
) -> Result<impl IntoResponse, ApiError> {
    validate_input(&payload)?;

    let command = CreateSupplierCommand {
        name: payload.name,
        contact_name: payload.contact_name,
        email: payload.email,
        phone: payload.phone,
        address: payload.address,
        category: payload.category,
        payment_terms: payload.payment_terms,
    };

    let supplier_id = state
        .services
        .suppliers
        .create_supplier(command)
        .await
        .map_err(map_service_error)?;

    info!("Supplier created: {}", supplier_id);

    created_response(serde_json::json!({
        "id": supplier_id,
        "message": "Supplier created successfully"
    }))
}

/// Get a supplier by ID
async fn get_supplier(
    State(state): State<Arc<AppState>>,
    Path(supplier_id): Path<Uuid>,
) -> Result<impl IntoResponse, ApiError> {
    let supplier = state
        .services
        .suppliers
        .get_supplier(&supplier_id)
        .await
        .map_err(map_service_error)?
        .ok_or_else(|| ApiError::NotFound { message: format!("Supplier with ID {} not found", supplier_id), error_code: None })?;

    success_response(supplier)
}

/// Update a supplier
async fn update_supplier(
    State(state): State<Arc<AppState>>,
    Path(supplier_id): Path<Uuid>,
    Json(payload): Json<UpdateSupplierRequest>,
) -> Result<impl IntoResponse, ApiError> {
    validate_input(&payload)?;

    let command = UpdateSupplierCommand {
        id: supplier_id,
        name: payload.name,
        contact_name: payload.contact_name,
        email: payload.email,
        phone: payload.phone,
        address: payload.address,
        category: payload.category,
        payment_terms: payload.payment_terms,
        rating: payload.rating,
    };

    state
        .services
        .suppliers
        .update_supplier(command)
        .await
        .map_err(map_service_error)?;

    info!("Supplier updated: {}", supplier_id);

    success_response(serde_json::json!({
        "message": "Supplier updated successfully"
    }))
}

/// Delete a supplier
async fn delete_supplier(
    State(state): State<Arc<AppState>>,
    Path(supplier_id): Path<Uuid>,
) -> Result<impl IntoResponse, ApiError> {
    let command = DeleteSupplierCommand { id: supplier_id };

    state
        .services
        .suppliers
        .delete_supplier(command)
        .await
        .map_err(map_service_error)?;

    info!("Supplier deleted: {}", supplier_id);

    no_content_response()
}

/// List all suppliers with pagination
async fn list_suppliers(
    State(state): State<Arc<AppState>>,
    Query(pagination): Query<PaginationParams>,
) -> Result<impl IntoResponse, ApiError> {
    let suppliers = state
        .services
        .suppliers
        .list_suppliers(pagination.per_page, pagination.offset())
        .await
        .map_err(map_service_error)?;

    success_response(suppliers)
}

/// Get suppliers by category
async fn get_suppliers_by_category(
    State(state): State<Arc<AppState>>,
    Path(category): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let suppliers = state
        .services
        .suppliers
        .get_suppliers_by_category(&category)
        .await
        .map_err(map_service_error)?;

    success_response(suppliers)
}

/// Get suppliers by minimum rating
async fn get_suppliers_by_min_rating(
    State(state): State<Arc<AppState>>,
    Path(min_rating): Path<f32>,
) -> Result<impl IntoResponse, ApiError> {
    let suppliers = state
        .services
        .suppliers
        .get_suppliers_by_min_rating(min_rating)
        .await
        .map_err(map_service_error)?;

    success_response(suppliers)
}

/// Get a supplier by name
async fn get_supplier_by_name(
    State(state): State<Arc<AppState>>,
    Query(params): Query<NameQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let supplier = state
        .services
        .suppliers
        .get_supplier_by_name(&params.name)
        .await
        .map_err(map_service_error)?
        .ok_or_else(|| {
            ApiError::NotFound { message: format!("Supplier with name '{}' not found", params.name), error_code: None }
        })?;

    success_response(supplier)
}

#[derive(Debug, Deserialize)]
pub struct NameQuery {
    pub name: String,
}

/// Creates the router for supplier endpoints
pub fn supplier_routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", post(create_supplier))
        .route("/", get(list_suppliers))
        .route("/:id", get(get_supplier))
        .route("/:id", put(update_supplier))
        .route("/:id", delete(delete_supplier))
        .route("/category/:category", get(get_suppliers_by_category))
        .route("/rating/:min_rating", get(get_suppliers_by_min_rating))
        .route("/name", get(get_supplier_by_name))
}
