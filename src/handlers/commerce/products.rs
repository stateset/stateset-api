use crate::handlers::common::{
    created_response, map_service_error, success_response, validate_input, PaginationParams,
};
use crate::{
    auth::AuthenticatedUser,
    errors::ApiError,
    services::commerce::product_catalog_service::{
        CreateProductInput, CreateVariantInput, ProductSearchQuery,
    },
    AppState,
};
use axum::{
    extract::{Json, Path, Query, State},
    routing::{get, post, put},
    Router,
};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use tracing::info;
use uuid::Uuid;
use validator::{Validate, ValidationError};

/// Custom validator for Decimal minimum value
fn validate_decimal_min_zero(value: &Decimal) -> Result<(), ValidationError> {
    if *value < Decimal::ZERO {
        return Err(ValidationError::new("decimal_min_zero"));
    }
    Ok(())
}

/// Creates the router for product endpoints
pub fn products_routes() -> Router<AppState> {
    Router::new()
        .route("/", get(list_products))
        .route("/", post(create_product))
        .route("/{id}", get(get_product))
        .route("/{id}/variants", get(get_product_variants))
        .route("/{id}/variants", post(create_variant))
        .route("/variants/{variant_id}/price", put(update_variant_price))
        .route("/search", get(search_products))
}

/// Create a new product
async fn create_product(
    _user: AuthenticatedUser,
    State(state): State<AppState>,
    Json(payload): Json<CreateProductRequest>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    validate_input(&payload)?;

    let input = CreateProductInput {
        name: payload.name,
        slug: payload.slug,
        description: payload.description,
        // The underlying entity doesn't use these enriched fields; provide defaults
        attributes: Vec::new(),
        seo: serde_json::json!({}),
    };

    let product = state
        .services
        .product_catalog
        .create_product(input)
        .await
        .map_err(map_service_error)?;

    Ok(created_response(ProductResponse::from(product)))
}

/// Get a product by ID
async fn get_product(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    let product = state
        .services
        .product_catalog
        .get_product(id)
        .await
        .map_err(map_service_error)?;

    Ok(success_response(ProductResponse::from(product)))
}

/// Get product variants
async fn get_product_variants(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    let variants = state
        .services
        .product_catalog
        .get_product_variants(id)
        .await
        .map_err(map_service_error)?;

    Ok(success_response(variants))
}

/// Create a product variant
async fn create_variant(
    _user: AuthenticatedUser,
    State(state): State<AppState>,
    Path(product_id): Path<Uuid>,
    Json(payload): Json<CreateVariantRequest>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    validate_input(&payload)?;

    let input = CreateVariantInput {
        product_id,
        sku: payload.sku,
        name: payload.name,
        price: payload.price,
        compare_at_price: payload.compare_at_price,
        cost: payload.cost,
        weight: payload.weight,
        dimensions: payload.dimensions,
        options: payload.options,
        inventory_tracking: payload.inventory_tracking.unwrap_or(true),
        position: payload.position.unwrap_or(0),
    };

    let variant = state
        .services
        .product_catalog
        .create_variant(input)
        .await
        .map_err(map_service_error)?;

    Ok(created_response(variant))
}

/// Update variant price
async fn update_variant_price(
    _user: AuthenticatedUser,
    State(state): State<AppState>,
    Path(variant_id): Path<Uuid>,
    Json(payload): Json<UpdatePriceRequest>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    validate_input(&payload)?;

    state
        .services
        .product_catalog
        .update_variant_price(variant_id, payload.price)
        .await
        .map_err(map_service_error)?;

    Ok(success_response(serde_json::json!({
        "message": "Price updated successfully"
    })))
}

/// Search products
async fn search_products(
    State(state): State<AppState>,
    Query(query): Query<ProductSearchQuery>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    let result = state
        .services
        .product_catalog
        .search_products(query)
        .await
        .map_err(map_service_error)?;

    Ok(success_response(result))
}

/// List all products with pagination
async fn list_products(
    State(state): State<AppState>,
    Query(params): Query<PaginationParams>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    let query = ProductSearchQuery {
        search: None,
        is_active: None,
        limit: Some(params.per_page),
        offset: Some(params.offset()),
    };

    let result = state
        .services
        .product_catalog
        .search_products(query)
        .await
        .map_err(map_service_error)?;

    Ok(success_response(result))
}

// Request/Response DTOs

#[derive(Debug, Deserialize, Validate)]
pub struct CreateProductRequest {
    #[validate(length(min = 1))]
    pub name: String,
    #[validate(length(min = 1))]
    pub slug: String,
    pub description: String,
}

#[derive(Debug, Serialize)]
pub struct ProductResponse {
    pub id: Uuid,
    pub name: String,
    pub sku: String,
    pub description: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: Option<chrono::DateTime<chrono::Utc>>,
}

impl From<crate::entities::commerce::ProductModel> for ProductResponse {
    fn from(model: crate::entities::commerce::ProductModel) -> Self {
        Self {
            id: model.id,
            name: model.name,
            sku: model.sku,
            description: model.description,
            created_at: model.created_at,
            updated_at: model.updated_at,
        }
    }
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreateVariantRequest {
    #[validate(length(min = 1))]
    pub sku: String,
    #[validate(length(min = 1))]
    pub name: String,
    #[validate(custom = "validate_decimal_min_zero")]
    pub price: Decimal,
    pub compare_at_price: Option<Decimal>,
    pub cost: Option<Decimal>,
    pub weight: Option<f64>,
    pub dimensions: Option<crate::entities::commerce::product_variant::Dimensions>,
    pub options: std::collections::HashMap<String, String>,
    pub inventory_tracking: Option<bool>,
    pub position: Option<i32>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdatePriceRequest {
    #[validate(custom = "validate_decimal_min_zero")]
    pub price: Decimal,
}
