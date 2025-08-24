use super::common::{
    created_response, map_service_error, no_content_response, success_response, validate_input,
    PaginationParams,
};
use crate::{
    auth::AuthenticatedUser,
    commands::billofmaterials::{
        audit_bom_command::AuditBOMCommand, create_bom_command::CreateBOMCommand,
        update_bom_command::UpdateBOMCommand,
    },
    errors::{ApiError, ServiceError},
    handlers::AppState,
    services::billofmaterials::BillOfMaterialsService,
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

/// Creates the router for BOM endpoints
pub fn bom_routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", post(create_bom))
        .route("/", get(list_boms))
        .route("/{id}", get(get_bom))
        .route("/{id}", put(update_bom))
        .route("/{id}/audit", post(audit_bom))
        .route("/{id}/components", get(get_bom_components))
        .route("/{id}/components", post(add_component_to_bom))
        .route("/{id}/components/{component_id}", delete(remove_component_from_bom))
}

// Request and response DTOs

#[derive(Debug, Deserialize, Validate)]
pub struct CreateBOMRequest {
    #[validate(length(min = 1, message = "Name cannot be empty"))]
    pub name: String,
    pub description: String,
    pub product_id: Uuid,
    pub revision: String,
    pub components: Vec<BOMComponentRequest>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct BOMComponentRequest {
    pub component_id: Uuid,
    #[validate(range(min = 1, message = "Quantity must be at least 1"))]
    pub quantity: i32,
    pub unit_of_measure: String,
    pub position: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateBOMRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub revision: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct AuditBOMRequest {
    #[validate(length(min = 1, message = "Auditor name cannot be empty"))]
    pub auditor: String,
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct AddComponentRequest {
    pub component_id: Uuid,
    #[validate(range(min = 1, message = "Quantity must be at least 1"))]
    pub quantity: i32,
    pub unit_of_measure: String,
    pub position: Option<String>,
    pub notes: Option<String>,
}

// Handler functions

/// Create a new BOM
async fn create_bom(
    State(state): State<Arc<AppState>>,
    user: AuthenticatedUser,
    Json(payload): Json<CreateBOMRequest>,
) -> Result<impl IntoResponse, ApiError> {
    validate_input(&payload)?;

    let components = payload
        .components
        .into_iter()
        .map(|c| {
            (
                c.component_id,
                c.quantity,
                c.unit_of_measure,
                c.position,
                c.notes,
            )
        })
        .collect();

    let command = CreateBOMCommand {
        name: payload.name,
        description: payload.description,
        product_id: payload.product_id,
        revision: payload.revision,
        components,
        created_by: user.user_id,
    };

    let bom_id = state
        .services
        .bill_of_materials
        .create_bom(command)
        .await
        .map_err(map_service_error)?;

    info!("BOM created: {}", bom_id);

    created_response(serde_json::json!({
        "id": bom_id,
        "message": "BOM created successfully"
    }))
}

/// Get a BOM by ID
async fn get_bom(
    State(state): State<Arc<AppState>>,
    Path(bom_id): Path<Uuid>,
) -> Result<impl IntoResponse, ApiError> {
    let bom = state
        .services
        .bill_of_materials
        .get_bom(&bom_id)
        .await
        .map_err(map_service_error)?
        .ok_or_else(|| ApiError::NotFound { message: format!("BOM with ID {} not found", bom_id), error_code: None })?;

    success_response(bom)
}

/// Update a BOM
async fn update_bom(
    State(state): State<Arc<AppState>>,
    Path(bom_id): Path<Uuid>,
    Json(payload): Json<UpdateBOMRequest>,
) -> Result<impl IntoResponse, ApiError> {
    validate_input(&payload)?;

    let command = UpdateBOMCommand {
        id: bom_id,
        name: payload.name,
        description: payload.description,
        revision: payload.revision,
        status: payload.status,
    };

    state
        .services
        .bill_of_materials
        .update_bom(command)
        .await
        .map_err(map_service_error)?;

    info!("BOM updated: {}", bom_id);

    success_response(serde_json::json!({
        "message": "BOM updated successfully"
    }))
}

/// Audit a BOM
async fn audit_bom(
    State(state): State<Arc<AppState>>,
    Path(bom_id): Path<Uuid>,
    Json(payload): Json<AuditBOMRequest>,
) -> Result<impl IntoResponse, ApiError> {
    validate_input(&payload)?;

    let command = AuditBOMCommand {
        id: bom_id,
        auditor: payload.auditor,
        notes: payload.notes,
    };

    state
        .services
        .bill_of_materials
        .audit_bom(command)
        .await
        .map_err(map_service_error)?;

    info!("BOM audited: {}", bom_id);

    success_response(serde_json::json!({
        "message": "BOM audit completed successfully"
    }))
}

/// List all BOMs with pagination
async fn list_boms(
    State(state): State<Arc<AppState>>,
    Query(params): Query<PaginationParams>,
) -> Result<impl IntoResponse, ApiError> {
    let (boms, total) = state
        .services
        .bill_of_materials
        .list_boms(params.page.unwrap_or(1), params.limit.unwrap_or(20))
        .await
        .map_err(map_service_error)?;

    success_response(serde_json::json!({
        "boms": boms,
        "total": total,
        "page": params.page.unwrap_or(1),
        "limit": params.limit.unwrap_or(20)
    }))
}

/// Get components for a BOM
async fn get_bom_components(
    State(state): State<Arc<AppState>>,
    Path(bom_id): Path<Uuid>,
) -> Result<impl IntoResponse, ApiError> {
    let components = state
        .services
        .bill_of_materials
        .get_bom_components(&bom_id)
        .await
        .map_err(map_service_error)?;

    success_response(components)
}

/// Add a component to a BOM
async fn add_component_to_bom(
    State(state): State<Arc<AppState>>,
    Path(bom_id): Path<Uuid>,
    Json(payload): Json<AddComponentRequest>,
) -> Result<impl IntoResponse, ApiError> {
    validate_input(&payload)?;

    let component_id = state
        .services
        .bill_of_materials
        .add_component_to_bom(
            &bom_id,
            &payload.component_id,
            payload.quantity,
            &payload.unit_of_measure,
            payload.position.as_deref(),
            payload.notes.as_deref(),
        )
        .await
        .map_err(map_service_error)?;

    info!(
        "Component {} added to BOM {}",
        payload.component_id, bom_id
    );

    created_response(serde_json::json!({
        "id": component_id,
        "message": "Component added to BOM successfully"
    }))
}

/// Remove a component from a BOM
async fn remove_component_from_bom(
    State(state): State<Arc<AppState>>,
    Path((bom_id, component_id)): Path<(Uuid, Uuid)>,
) -> Result<impl IntoResponse, ApiError> {
    state
        .services
        .bill_of_materials
        .remove_component_from_bom(&bom_id, &component_id)
        .await
        .map_err(map_service_error)?;

    info!("Component {} removed from BOM {}", component_id, bom_id);

    no_content_response()
}
