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
    services::suppliers::SupplierService,
    commands::suppliers::{
        create_supplier_command::CreateSupplierCommand,
        update_supplier_command::UpdateSupplierCommand,
        delete_supplier_command::DeleteSupplierCommand,
    },
    main::AppState,
};
use serde::{Serialize, Deserialize};
use validator::Validate;
use tracing::info;
use super::common::{validate_input, map_service_error, success_response, created_response, no_content_response, PaginationParams};

// Request and response DTOs

#[derive(Debug, Deserialize, Validate)]
pub struct CreateSupplierRequest {
    #[validate(length(min = 1, message = "Name cannot be empty"))]
    pub name: String,
    #[validate(length(min = 1, message = "Contact name cannot be empty"))]
    pub contact_name: String,
    #[validate(email)]
    pub email: String,
    #[validate(length(min = 10, message = "Phone number must be at least 10 characters"))]
    pub phone: String,
    pub address: String,
    pub category: String,
    pub payment_terms: String,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateSupplierRequest {
    pub name: Option<String>,
    pub contact_name: Option<String>,
    #[validate(email)]
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
    AuthenticatedUser(user): AuthenticatedUser,
    Json(payload): Json<CreateSupplierRequest>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
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
    
    let supplier_id = state.services.suppliers
        .create_supplier(command)
        .await
        .map_err(map_service_error)?;
    
    info\!("Supplier created: {}", supplier_id);
    
    created_response(serde_json::json\!({
        "id": supplier_id,
        "message": "Supplier created successfully"
    }))
}

/// Get a supplier by ID
async fn get_supplier(
    State(state): State<Arc<AppState>>,
    Path(supplier_id): Path<Uuid>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    let supplier = state.services.suppliers
        .get_supplier(&supplier_id)
        .await
        .map_err(map_service_error)?
        .ok_or_else(|| ApiError::NotFound(format\!("Supplier with ID {} not found", supplier_id)))?;
    
    success_response(supplier)
}

/// Update a supplier
async fn update_supplier(
    State(state): State<Arc<AppState>>,
    Path(supplier_id): Path<Uuid>,
    Json(payload): Json<UpdateSupplierRequest>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
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
    
    state.services.suppliers
        .update_supplier(command)
        .await
        .map_err(map_service_error)?;
    
    info\!("Supplier updated: {}", supplier_id);
    
    success_response(serde_json::json\!({
        "message": "Supplier updated successfully"
    }))
}

/// Delete a supplier
async fn delete_supplier(
    State(state): State<Arc<AppState>>,
    Path(supplier_id): Path<Uuid>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    let command = DeleteSupplierCommand { id: supplier_id };
    
    state.services.suppliers
        .delete_supplier(command)
        .await
        .map_err(map_service_error)?;
    
    info\!("Supplier deleted: {}", supplier_id);
    
    no_content_response()
}

/// List all suppliers with pagination
async fn list_suppliers(
    State(state): State<Arc<AppState>>,
    Query(pagination): Query<PaginationParams>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    let suppliers = state.services.suppliers
        .list_suppliers(pagination.per_page, pagination.offset())
        .await
        .map_err(map_service_error)?;
    
    success_response(suppliers)
}

/// Get suppliers by category
async fn get_suppliers_by_category(
    State(state): State<Arc<AppState>>,
    Path(category): Path<String>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    let suppliers = state.services.suppliers
        .get_suppliers_by_category(&category)
        .await
        .map_err(map_service_error)?;
    
    success_response(suppliers)
}

/// Get suppliers by minimum rating
async fn get_suppliers_by_min_rating(
    State(state): State<Arc<AppState>>,
    Path(min_rating): Path<f32>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    let suppliers = state.services.suppliers
        .get_suppliers_by_min_rating(min_rating)
        .await
        .map_err(map_service_error)?;
    
    success_response(suppliers)
}

/// Get a supplier by name
async fn get_supplier_by_name(
    State(state): State<Arc<AppState>>,
    Query(params): Query<NameQuery>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    let supplier = state.services.suppliers
        .get_supplier_by_name(&params.name)
        .await
        .map_err(map_service_error)?
        .ok_or_else(|| ApiError::NotFound(format\!("Supplier with name '{}' not found", params.name)))?;
    
    success_response(supplier)
}

#[derive(Debug, Deserialize)]
pub struct NameQuery {
    pub name: String,
}

/// Creates the router for supplier endpoints
pub fn supplier_routes() -> Router {
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
