use super::common::{
    created_response, map_service_error, no_content_response, success_response, validate_input,
    PaginationParams,
};
use crate::{
    auth::AuthenticatedUser,
    errors::ApiError,
    handlers::AppState,
    services::billofmaterials::{
        AuditBomInput, BillOfMaterialsService, CreateBomComponentInput, CreateBomInput,
        UpdateBomInput,
    },
};
use axum::{
    extract::{Json, Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, post, put},
    Router,
};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use tracing::info;
use uuid::Uuid;
use validator::Validate;

/// Creates the router for BOM endpoints
pub fn bom_routes() -> Router<AppState> {
    Router::new()
        .route("/", post(create_bom))
        .route("/", get(list_boms))
        .route("/{id}", get(get_bom))
        .route("/{id}", put(update_bom))
        .route("/{id}/audit", post(audit_bom))
        .route("/{id}/components", get(get_bom_components))
        .route("/{id}/components", post(add_component_to_bom))
        .route(
            "/{id}/components/{component_id}",
            delete(remove_component_from_bom),
        )
}

// Request and response DTOs

#[derive(Debug, Deserialize, Validate)]
pub struct CreateBOMRequest {
    pub name: String,
    pub description: String,
    pub product_id: Uuid,
    pub revision: String,
    pub components: Vec<BOMComponentRequest>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct BOMComponentRequest {
    pub component_id: Uuid,

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
    pub auditor: String,
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct AddComponentRequest {
    pub component_id: Uuid,

    pub quantity: i32,
    pub unit_of_measure: String,
    pub position: Option<String>,
    pub notes: Option<String>,
}

// Handler functions

/// Create a new BOM
async fn create_bom(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Json(payload): Json<CreateBOMRequest>,
) -> Result<impl IntoResponse, ApiError> {
    validate_input(&payload)?;

    let created_by = Uuid::parse_str(&user.user_id).ok();

    let component_inputs = payload
        .components
        .into_iter()
        .map(|component| CreateBomComponentInput {
            component_product_id: Some(component.component_id),
            component_item_id: None,
            quantity: Decimal::from(component.quantity),
            unit_of_measure: component.unit_of_measure,
            position: component.position,
            notes: component.notes,
        })
        .collect();

    let input = CreateBomInput {
        product_id: payload.product_id,
        item_master_id: None,
        name: payload.name,
        description: Some(payload.description),
        revision: payload.revision,
        components: component_inputs,
        created_by,
        lifecycle_status: None,
        metadata: None,
        bom_number: None,
    };

    let bom_id = state
        .services
        .bill_of_materials
        .create_bom(input)
        .await
        .map_err(map_service_error)?;

    info!("BOM created: {}", bom_id);

    Ok(created_response(serde_json::json!({
        "id": bom_id,
        "message": "BOM created successfully"
    })))
}

/// Get a BOM by ID
async fn get_bom(
    State(state): State<AppState>,
    Path(bom_id): Path<Uuid>,
) -> Result<impl IntoResponse, ApiError> {
    let maybe_bom = state
        .services
        .bill_of_materials
        .get_bom(&bom_id)
        .await
        .map_err(map_service_error)?;

    if let Some(bom) = maybe_bom {
        Ok(success_response(bom))
    } else {
        Err(ApiError::NotFound(format!(
            "BOM with ID {} not found",
            bom_id
        )))
    }
}

/// Update a BOM
async fn update_bom(
    State(state): State<AppState>,
    Path(bom_id): Path<Uuid>,
    user: AuthenticatedUser,
    Json(payload): Json<UpdateBOMRequest>,
) -> Result<impl IntoResponse, ApiError> {
    validate_input(&payload)?;

    let updated_by = Uuid::parse_str(&user.user_id).ok();

    let input = UpdateBomInput {
        name: payload.name,
        description: payload.description,
        revision: payload.revision,
        lifecycle_status: payload.status,
        metadata: None,
        updated_by,
    };

    state
        .services
        .bill_of_materials
        .update_bom(bom_id, input)
        .await
        .map_err(map_service_error)?;

    info!("BOM updated: {}", bom_id);

    Ok(success_response(serde_json::json!({
        "message": "BOM updated successfully"
    })))
}

/// Audit a BOM
async fn audit_bom(
    State(state): State<AppState>,
    Path(bom_id): Path<Uuid>,
    Json(payload): Json<AuditBOMRequest>,
) -> Result<impl IntoResponse, ApiError> {
    validate_input(&payload)?;

    let parsed_user = Uuid::parse_str(&payload.auditor).ok();
    let notes = match (payload.notes.clone(), parsed_user) {
        (Some(note), Some(_)) => Some(note),
        (Some(note), None) => Some(format!("{} (auditor: {})", note, payload.auditor)),
        (None, Some(_)) => None,
        (None, None) => Some(format!("Audit recorded by {}", payload.auditor)),
    };

    let input = AuditBomInput {
        event_type: "audit".to_string(),
        user_id: parsed_user,
        notes,
        event_at: None,
    };

    state
        .services
        .bill_of_materials
        .audit_bom(bom_id, input)
        .await
        .map_err(map_service_error)?;

    info!("BOM audited: {}", bom_id);

    Ok(success_response(serde_json::json!({
        "message": "BOM audit completed successfully"
    })))
}

/// List all BOMs with pagination
async fn list_boms(
    State(state): State<AppState>,
    Query(params): Query<PaginationParams>,
) -> Result<impl IntoResponse, ApiError> {
    let page = params.page.max(1);
    let per_page = params.per_page.max(1);

    let (boms, total) = state
        .services
        .bill_of_materials
        .list_boms(page, per_page)
        .await
        .map_err(map_service_error)?;

    Ok(success_response(serde_json::json!({
        "boms": boms,
        "total": total,
        "page": page,
        "per_page": per_page
    })))
}

/// Get components for a BOM
async fn get_bom_components(
    State(state): State<AppState>,
    Path(bom_id): Path<Uuid>,
) -> Result<impl IntoResponse, ApiError> {
    let components = state
        .services
        .bill_of_materials
        .get_bom_components(&bom_id)
        .await
        .map_err(map_service_error)?;

    Ok(success_response(components))
}

/// Add a component to a BOM
async fn add_component_to_bom(
    State(state): State<AppState>,
    Path(bom_id): Path<Uuid>,
    Json(payload): Json<AddComponentRequest>,
) -> Result<impl IntoResponse, ApiError> {
    validate_input(&payload)?;

    let component_id = state
        .services
        .bill_of_materials
        .add_component_to_bom(
            &bom_id,
            CreateBomComponentInput {
                component_product_id: Some(payload.component_id),
                component_item_id: None,
                quantity: Decimal::from(payload.quantity),
                unit_of_measure: payload.unit_of_measure.clone(),
                position: payload.position.clone(),
                notes: payload.notes.clone(),
            },
        )
        .await
        .map_err(map_service_error)?;

    info!("Component {} added to BOM {}", payload.component_id, bom_id);

    Ok(created_response(serde_json::json!({
        "id": component_id,
        "message": "Component added to BOM successfully"
    })))
}

/// Remove a component from a BOM
async fn remove_component_from_bom(
    State(state): State<AppState>,
    Path((bom_id, component_id)): Path<(Uuid, Uuid)>,
) -> Result<impl IntoResponse, ApiError> {
    state
        .services
        .bill_of_materials
        .remove_component_from_bom(&bom_id, &component_id)
        .await
        .map_err(map_service_error)?;

    info!("Component {} removed from BOM {}", component_id, bom_id);

    Ok(no_content_response())
}
