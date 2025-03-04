use axum::{
    routing::{get, post, put, delete},
    extract::{State, Path, Json},
    Router,
};
use std::sync::Arc;
use uuid::Uuid;
use crate::{
    auth::AuthenticatedUser,
    errors::{ApiError, ServiceError},
    services::billofmaterials::BillOfMaterialsService,
    commands::billofmaterials::{
        create_bom_command::CreateBomCommand,
        update_bom_command::UpdateBomCommand,
        delete_bom_command::DeleteBomCommand,
        add_component_to_bom_command::AddComponentToBomCommand,
        remove_component_from_bom_command::RemoveComponentFromBomCommand,
    },
    main::AppState,
};
use serde::{Serialize, Deserialize};
use validator::Validate;
use tracing::info;
use super::common::{validate_input, map_service_error, success_response, created_response, no_content_response, PaginationParams};

// Request and response DTOs

#[derive(Debug, Deserialize, Validate)]
pub struct CreateBomRequest {
    pub product_id: Uuid,
    #[validate(length(min = 1, message = "Name cannot be empty"))]
    pub name: String,
    pub description: Option<String>,
    #[validate(length(min = 1, message = "Version cannot be empty"))]
    pub version: String,
    pub components: Vec<BomComponentRequest>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct BomComponentRequest {
    pub component_id: Uuid,
    #[validate(range(min = 1, message = "Quantity must be at least 1"))]
    pub quantity: i32,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateBomRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub version: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct AddComponentRequest {
    pub component_id: Uuid,
    #[validate(range(min = 1, message = "Quantity must be at least 1"))]
    pub quantity: i32,
}

// Handler functions

/// Create a new bill of materials
async fn create_bom(
    State(state): State<Arc<AppState>>,
    AuthenticatedUser(user): AuthenticatedUser,
    Json(payload): Json<CreateBomRequest>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    validate_input(&payload)?;
    
    let command = CreateBomCommand {
        product_id: payload.product_id,
        name: payload.name,
        description: payload.description.unwrap_or_default(),
        version: payload.version,
        components: payload.components.into_iter().map(|c| (c.component_id, c.quantity)).collect(),
    };
    
    let bom_id = state.services.bill_of_materials
        .create_bom(command)
        .await
        .map_err(map_service_error)?;
    
    info!("Bill of materials created: {}", bom_id);
    
    created_response(serde_json::json!({
        "id": bom_id,
        "message": "Bill of materials created successfully"
    }))
}

/// Get a bill of materials by ID
async fn get_bom(
    State(state): State<Arc<AppState>>,
    Path(bom_id): Path<Uuid>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    let bom = state.services.bill_of_materials
        .get_bom(&bom_id)
        .await
        .map_err(map_service_error)?
        .ok_or_else(|| ApiError::NotFound(format!("Bill of materials with ID {} not found", bom_id)))?;
    
    success_response(bom)
}

/// Update a bill of materials
async fn update_bom(
    State(state): State<Arc<AppState>>,
    Path(bom_id): Path<Uuid>,
    Json(payload): Json<UpdateBomRequest>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    validate_input(&payload)?;
    
    let command = UpdateBomCommand {
        id: bom_id,
        name: payload.name,
        description: payload.description,
        version: payload.version,
    };
    
    state.services.bill_of_materials
        .update_bom(command)
        .await
        .map_err(map_service_error)?;
    
    info!("Bill of materials updated: {}", bom_id);
    
    success_response(serde_json::json!({
        "message": "Bill of materials updated successfully"
    }))
}

/// Delete a bill of materials
async fn delete_bom(
    State(state): State<Arc<AppState>>,
    Path(bom_id): Path<Uuid>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    let command = DeleteBomCommand { id: bom_id };
    
    state.services.bill_of_materials
        .delete_bom(command)
        .await
        .map_err(map_service_error)?;
    
    info!("Bill of materials deleted: {}", bom_id);
    
    no_content_response()
}

/// Add a component to a bill of materials
async fn add_component(
    State(state): State<Arc<AppState>>,
    Path(bom_id): Path<Uuid>,
    Json(payload): Json<AddComponentRequest>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    validate_input(&payload)?;
    
    let command = AddComponentToBomCommand {
        bom_id,
        component_id: payload.component_id,
        quantity: payload.quantity,
    };
    
    state.services.bill_of_materials
        .add_component_to_bom(command)
        .await
        .map_err(map_service_error)?;
    
    info!("Component added to BOM {}: {}", bom_id, payload.component_id);
    
    success_response(serde_json::json!({
        "message": "Component added successfully"
    }))
}

/// Remove a component from a bill of materials
async fn remove_component(
    State(state): State<Arc<AppState>>,
    Path((bom_id, component_id)): Path<(Uuid, Uuid)>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    let command = RemoveComponentFromBomCommand {
        bom_id,
        component_id,
    };
    
    state.services.bill_of_materials
        .remove_component_from_bom(command)
        .await
        .map_err(map_service_error)?;
    
    info!("Component removed from BOM {}: {}", bom_id, component_id);
    
    no_content_response()
}

/// Get all BOMs for a product
async fn get_boms_for_product(
    State(state): State<Arc<AppState>>,
    Path(product_id): Path<Uuid>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    let boms = state.services.bill_of_materials
        .get_boms_for_product(&product_id)
        .await
        .map_err(map_service_error)?;
    
    success_response(boms)
}

/// Get a specific BOM version for a product
async fn get_bom_by_version(
    State(state): State<Arc<AppState>>,
    Path((product_id, version)): Path<(Uuid, String)>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    let bom = state.services.bill_of_materials
        .get_bom_by_version(&product_id, &version)
        .await
        .map_err(map_service_error)?
        .ok_or_else(|| {
            ApiError::NotFound(format!("Bill of materials for product {} with version {} not found", product_id, version))
        })?;
    
    success_response(bom)
}

/// Creates the router for bill of materials endpoints
pub fn bom_routes() -> Router {
    Router::new()
        .route("/", post(create_bom))
        .route("/:id", get(get_bom))
        .route("/:id", put(update_bom))
        .route("/:id", delete(delete_bom))
        .route("/:id/components", post(add_component))
        .route("/:id/components/:component_id", delete(remove_component))
        .route("/product/:product_id", get(get_boms_for_product))
        .route("/product/:product_id/version/:version", get(get_bom_by_version))
}