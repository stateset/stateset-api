use crate::handlers::common::{
    created_response, map_service_error, success_response, validate_input, PaginatedResponse,
    PaginationParams,
};
use crate::{
    auth::AuthenticatedUser,
    errors::ApiError,
    services::commerce::product_catalog_service::{
        CreateProductInput, CreateVariantInput, ProductSearchQuery, UpdateProductInput,
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
use uuid::Uuid;
use validator::{Validate, ValidationError};

const DEFAULT_CURRENCY: &str = "USD";
const MAX_PAGE_SIZE: u64 = 100;

/// Custom validator for Decimal minimum value
fn validate_decimal_min_zero(value: &Decimal) -> Result<(), ValidationError> {
    if *value < Decimal::ZERO {
        return Err(ValidationError::new("decimal_min_zero"));
    }
    Ok(())
}

fn normalize_string(value: String) -> String {
    value.trim().to_string()
}

fn normalize_optional_string(value: Option<String>) -> Option<String> {
    value
        .map(|v| v.trim().to_string())
        .and_then(|v| if v.is_empty() { None } else { Some(v) })
}

fn ensure_decimal_non_negative(value: &Decimal, field: &str) -> Result<(), ApiError> {
    if *value < Decimal::ZERO {
        Err(ApiError::ValidationError(format!(
            "{field} cannot be negative"
        )))
    } else {
        Ok(())
    }
}

fn ensure_i32_non_negative(value: i32, field: &str) -> Result<(), ApiError> {
    if value < 0 {
        Err(ApiError::ValidationError(format!(
            "{field} cannot be negative"
        )))
    } else {
        Ok(())
    }
}

/// Creates the router for product endpoints
pub fn products_routes() -> Router<AppState> {
    Router::new()
        .route("/", get(list_products))
        .route("/", post(create_product))
        .route("/{id}", get(get_product))
        .route("/{id}", put(update_product))
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

    let CreateProductRequest {
        name,
        sku,
        description,
        price,
        currency,
        is_active,
        is_digital,
        image_url,
        brand,
        manufacturer,
        weight_kg,
        dimensions_cm,
        tags,
        cost_price,
        msrp,
        tax_rate,
        meta_title,
        meta_description,
        reorder_point,
    } = payload;

    let name = normalize_string(name);
    if name.is_empty() {
        return Err(ApiError::ValidationError(
            "Product name cannot be blank".to_string(),
        ));
    }

    let sku = normalize_string(sku);
    if sku.is_empty() {
        return Err(ApiError::ValidationError(
            "SKU cannot be blank".to_string(),
        ));
    }

    let description = normalize_optional_string(description);
    let image_url = normalize_optional_string(image_url);
    let brand = normalize_optional_string(brand);
    let manufacturer = normalize_optional_string(manufacturer);
    let dimensions_cm = normalize_optional_string(dimensions_cm);
    let tags = normalize_optional_string(tags);
    let meta_title = normalize_optional_string(meta_title);
    let meta_description = normalize_optional_string(meta_description);

    if let Some(value) = price.as_ref() {
        ensure_decimal_non_negative(value, "price")?;
    }
    if let Some(value) = cost_price.as_ref() {
        ensure_decimal_non_negative(value, "cost_price")?;
    }
    if let Some(value) = msrp.as_ref() {
        ensure_decimal_non_negative(value, "msrp")?;
    }
    if let Some(value) = tax_rate.as_ref() {
        ensure_decimal_non_negative(value, "tax_rate")?;
    }
    if let Some(value) = weight_kg.as_ref() {
        ensure_decimal_non_negative(value, "weight_kg")?;
    }
    if let Some(value) = reorder_point.as_ref() {
        ensure_i32_non_negative(*value, "reorder_point")?;
    }

    let currency = currency
        .map(|value| value.trim().to_ascii_uppercase())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| DEFAULT_CURRENCY.to_string());

    let input = CreateProductInput {
        name,
        sku,
        description,
        price: price.unwrap_or_else(|| Decimal::ZERO),
        currency,
        is_active: is_active.unwrap_or(true),
        is_digital: is_digital.unwrap_or(false),
        image_url,
        brand,
        manufacturer,
        weight_kg,
        dimensions_cm,
        tags,
        cost_price,
        msrp,
        tax_rate,
        meta_title,
        meta_description,
        reorder_point,
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

/// Update a product
async fn update_product(
    _user: AuthenticatedUser,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateProductRequest>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    validate_input(&payload)?;

    let UpdateProductRequest {
        name,
        sku,
        description,
        price,
        currency,
        is_active,
        is_digital,
        image_url,
        brand,
        manufacturer,
        weight_kg,
        dimensions_cm,
        tags,
        cost_price,
        msrp,
        tax_rate,
        meta_title,
        meta_description,
        reorder_point,
    } = payload;

    let name = name
        .map(normalize_string)
        .map(|value| {
            if value.is_empty() {
                Err(ApiError::ValidationError(
                    "Product name cannot be blank".to_string(),
                ))
            } else {
                Ok(value)
            }
        })
        .transpose()?;

    let sku = sku
        .map(normalize_string)
        .map(|value| {
            if value.is_empty() {
                Err(ApiError::ValidationError(
                    "SKU cannot be blank".to_string(),
                ))
            } else {
                Ok(value)
            }
        })
        .transpose()?;

    let description = normalize_optional_string(description);
    let image_url = normalize_optional_string(image_url);
    let brand = normalize_optional_string(brand);
    let manufacturer = normalize_optional_string(manufacturer);
    let dimensions_cm = normalize_optional_string(dimensions_cm);
    let tags = normalize_optional_string(tags);
    let meta_title = normalize_optional_string(meta_title);
    let meta_description = normalize_optional_string(meta_description);

    if let Some(ref value) = price {
        ensure_decimal_non_negative(value, "price")?;
    }
    if let Some(ref value) = cost_price {
        ensure_decimal_non_negative(value, "cost_price")?;
    }
    if let Some(ref value) = msrp {
        ensure_decimal_non_negative(value, "msrp")?;
    }
    if let Some(ref value) = tax_rate {
        ensure_decimal_non_negative(value, "tax_rate")?;
    }
    if let Some(ref value) = weight_kg {
        ensure_decimal_non_negative(value, "weight_kg")?;
    }
    if let Some(value) = reorder_point.as_ref() {
        ensure_i32_non_negative(*value, "reorder_point")?;
    }

    let currency = currency
        .map(|value| value.trim().to_ascii_uppercase())
        .filter(|value| !value.is_empty());

    let input = UpdateProductInput {
        name,
        sku,
        description,
        price,
        currency,
        is_active,
        is_digital,
        image_url,
        brand,
        manufacturer,
        weight_kg,
        dimensions_cm,
        tags,
        cost_price,
        msrp,
        tax_rate,
        meta_title,
        meta_description,
        reorder_point,
    };

    let product = state
        .services
        .product_catalog
        .update_product(id, input)
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
    Query(mut query): Query<ProductSearchQuery>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    if let Some(limit) = query.limit {
        if limit == 0 {
            return Err(ApiError::ValidationError(
                "limit must be greater than zero".to_string(),
            ));
        }
        if limit > MAX_PAGE_SIZE {
            query.limit = Some(MAX_PAGE_SIZE);
        }
    }

    let limit = query.limit;
    let offset = query.offset;

    let result = state
        .services
        .product_catalog
        .search_products(query)
        .await
        .map_err(map_service_error)?;

    let products = result
        .products
        .into_iter()
        .map(ProductResponse::from)
        .collect();

    Ok(success_response(ProductSearchResponse {
        products,
        total: result.total,
        limit,
        offset,
    }))
}

/// List all products with pagination
async fn list_products(
    State(state): State<AppState>,
    Query(params): Query<PaginationParams>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    if params.page == 0 {
        return Err(ApiError::ValidationError(
            "page must be greater than zero".to_string(),
        ));
    }
    if params.per_page == 0 {
        return Err(ApiError::ValidationError(
            "per_page must be greater than zero".to_string(),
        ));
    }
    if params.per_page > MAX_PAGE_SIZE {
        return Err(ApiError::ValidationError(format!(
            "per_page cannot exceed {MAX_PAGE_SIZE}"
        )));
    }

    let page = params.page;
    let per_page = params.per_page;
    let offset = (page.saturating_sub(1)).saturating_mul(per_page);

    let query = ProductSearchQuery {
        search: None,
        is_active: None,
        limit: Some(per_page),
        offset: Some(offset),
    };

    let result = state
        .services
        .product_catalog
        .search_products(query)
        .await
        .map_err(map_service_error)?;

    let products = result
        .products
        .into_iter()
        .map(ProductResponse::from)
        .collect();

    Ok(success_response(PaginatedResponse::new(
        products,
        page,
        per_page,
        result.total,
    )))
}

// Request/Response DTOs

#[derive(Debug, Deserialize, Validate)]
pub struct CreateProductRequest {
    #[validate(length(min = 1))]
    pub name: String,
    #[serde(alias = "slug")]
    #[validate(length(min = 1))]
    pub sku: String,
    #[serde(default)]
    #[validate(length(max = 2000))]
    pub description: Option<String>,
    #[serde(default)]
    pub price: Option<Decimal>,
    #[serde(default)]
    #[validate(length(equal = 3))]
    pub currency: Option<String>,
    #[serde(default)]
    pub is_active: Option<bool>,
    #[serde(default)]
    pub is_digital: Option<bool>,
    #[serde(default)]
    #[validate(url)]
    pub image_url: Option<String>,
    #[serde(default)]
    pub brand: Option<String>,
    #[serde(default)]
    pub manufacturer: Option<String>,
    #[serde(default)]
    pub weight_kg: Option<Decimal>,
    #[serde(default)]
    pub dimensions_cm: Option<String>,
    #[serde(default)]
    pub tags: Option<String>,
    #[serde(default)]
    pub cost_price: Option<Decimal>,
    #[serde(default)]
    pub msrp: Option<Decimal>,
    #[serde(default)]
    pub tax_rate: Option<Decimal>,
    #[serde(default)]
    pub meta_title: Option<String>,
    #[serde(default)]
    pub meta_description: Option<String>,
    #[serde(default)]
    pub reorder_point: Option<i32>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateProductRequest {
    #[validate(length(min = 1))]
    pub name: Option<String>,
    #[serde(alias = "slug")]
    #[validate(length(min = 1))]
    pub sku: Option<String>,
    #[serde(default)]
    #[validate(length(max = 2000))]
    pub description: Option<String>,
    #[serde(default)]
    pub price: Option<Decimal>,
    #[serde(default)]
    #[validate(length(equal = 3))]
    pub currency: Option<String>,
    pub is_active: Option<bool>,
    pub is_digital: Option<bool>,
    #[serde(default)]
    #[validate(url)]
    pub image_url: Option<String>,
    #[serde(default)]
    pub brand: Option<String>,
    #[serde(default)]
    pub manufacturer: Option<String>,
    #[serde(default)]
    pub weight_kg: Option<Decimal>,
    #[serde(default)]
    pub dimensions_cm: Option<String>,
    #[serde(default)]
    pub tags: Option<String>,
    #[serde(default)]
    pub cost_price: Option<Decimal>,
    #[serde(default)]
    pub msrp: Option<Decimal>,
    #[serde(default)]
    pub tax_rate: Option<Decimal>,
    #[serde(default)]
    pub meta_title: Option<String>,
    #[serde(default)]
    pub meta_description: Option<String>,
    #[serde(default)]
    pub reorder_point: Option<i32>,
}

#[derive(Debug, Serialize)]
pub struct ProductResponse {
    pub id: Uuid,
    pub name: String,
    pub sku: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub price: Decimal,
    pub currency: String,
    pub is_active: bool,
    pub is_digital: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub brand: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub manufacturer: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub weight_kg: Option<Decimal>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dimensions_cm: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost_price: Option<Decimal>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub msrp: Option<Decimal>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tax_rate: Option<Decimal>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meta_title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meta_description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reorder_point: Option<i32>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<chrono::DateTime<chrono::Utc>>,
}

impl From<crate::entities::commerce::ProductModel> for ProductResponse {
    fn from(model: crate::entities::commerce::ProductModel) -> Self {
        Self {
            id: model.id,
            name: model.name,
            sku: model.sku,
            description: model.description,
            price: model.price,
            currency: model.currency,
            is_active: model.is_active,
            is_digital: model.is_digital,
            image_url: model.image_url,
            brand: model.brand,
            manufacturer: model.manufacturer,
            weight_kg: model.weight_kg,
            dimensions_cm: model.dimensions_cm,
            tags: model.tags,
            cost_price: model.cost_price,
            msrp: model.msrp,
            tax_rate: model.tax_rate,
            meta_title: model.meta_title,
            meta_description: model.meta_description,
            reorder_point: model.reorder_point,
            created_at: model.created_at,
            updated_at: model.updated_at,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ProductSearchResponse {
    pub products: Vec<ProductResponse>,
    pub total: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offset: Option<u64>,
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
