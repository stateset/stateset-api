use crate::auth::consts as perm;
use crate::auth::{AuthRouterExt, AuthenticatedUser};
use crate::entities::commerce::product_variant;
use crate::handlers::common::{
    created_response, map_service_error, no_content_response, success_response, validate_input,
    PaginatedResponse, PaginationParams,
};
use crate::{
    errors::ApiError,
    services::commerce::product_catalog_service::{
        CreateProductInput, CreateVariantInput, ProductSearchQuery, UpdateProductInput,
    },
    AppState,
};
use axum::{
    extract::{Json, Path, Query, State},
    routing::{delete, get, post, put},
    Router,
};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::{IntoParams, ToSchema};
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
    let protected = Router::new()
        .route("/", post(create_product))
        .route("/:id", put(update_product))
        .route("/:id/variants", post(create_variant))
        .route("/variants/:variant_id", delete(delete_variant))
        .route("/variants/:variant_id/price", put(update_variant_price))
        .with_permission(perm::INVENTORY_ADJUST);

    Router::new()
        .route("/", get(list_products))
        .route("/:id", get(get_product))
        .route("/:id/variants", get(get_product_variants))
        .route("/variants/:variant_id", get(get_variant))
        .route("/search", get(search_products))
        .merge(protected)
}

/// Create a new product
#[utoipa::path(
    post,
    path = "/api/v1/products",
    request_body = CreateProductRequest,
    responses(
        (status = 201, description = "Product created", body = crate::ApiResponse<ProductResponse>),
        (status = 400, description = "Invalid payload", body = crate::errors::ErrorResponse),
        (status = 401, description = "Unauthorized", body = crate::errors::ErrorResponse),
        (status = 403, description = "Forbidden", body = crate::errors::ErrorResponse)
    ),
    security(("Bearer" = []), ("ApiKey" = [])),
    tag = "Products"
)]
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
        return Err(ApiError::ValidationError("SKU cannot be blank".to_string()));
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
#[utoipa::path(
    get,
    path = "/api/v1/products/:id",
    params(
        ("id" = Uuid, Path, description = "Product ID")
    ),
    responses(
        (status = 200, description = "Product retrieved", body = crate::ApiResponse<ProductResponse>),
        (status = 404, description = "Product not found", body = crate::errors::ErrorResponse)
    ),
    security(("Bearer" = []), ("ApiKey" = [])),
    tag = "Products"
)]
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
#[utoipa::path(
    put,
    path = "/api/v1/products/:id",
    params(
        ("id" = Uuid, Path, description = "Product ID")
    ),
    request_body = UpdateProductRequest,
    responses(
        (status = 200, description = "Product updated", body = crate::ApiResponse<ProductResponse>),
        (status = 400, description = "Invalid payload", body = crate::errors::ErrorResponse),
        (status = 404, description = "Product not found", body = crate::errors::ErrorResponse)
    ),
    security(("Bearer" = []), ("ApiKey" = [])),
    tag = "Products"
)]
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
                Err(ApiError::ValidationError("SKU cannot be blank".to_string()))
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
#[utoipa::path(
    get,
    path = "/api/v1/products/:id/variants",
    params(
        ("id" = Uuid, Path, description = "Product ID")
    ),
    responses(
        (status = 200, description = "Variants retrieved", body = crate::ApiResponse<Vec<VariantResponse>>),
        (status = 404, description = "Product not found", body = crate::errors::ErrorResponse)
    ),
    security(("Bearer" = []), ("ApiKey" = [])),
    tag = "Products"
)]
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

    let variants: Vec<VariantResponse> = variants.into_iter().map(VariantResponse::from).collect();

    Ok(success_response(variants))
}

/// Create a product variant
#[utoipa::path(
    post,
    path = "/api/v1/products/:id/variants",
    params(
        ("id" = Uuid, Path, description = "Product ID")
    ),
    request_body = CreateVariantRequest,
    responses(
        (status = 201, description = "Variant created", body = crate::ApiResponse<VariantResponse>),
        (status = 400, description = "Invalid payload", body = crate::errors::ErrorResponse),
        (status = 404, description = "Product not found", body = crate::errors::ErrorResponse)
    ),
    security(("Bearer" = []), ("ApiKey" = [])),
    tag = "Products"
)]
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

    Ok(created_response(VariantResponse::from(variant)))
}

/// Get a single variant
#[utoipa::path(
    get,
    path = "/api/v1/products/variants/:variant_id",
    params(
        ("variant_id" = Uuid, Path, description = "Variant ID")
    ),
    responses(
        (status = 200, description = "Variant retrieved", body = crate::ApiResponse<VariantResponse>),
        (status = 404, description = "Variant not found", body = crate::errors::ErrorResponse)
    ),
    security(("Bearer" = []), ("ApiKey" = [])),
    tag = "Products"
)]
async fn get_variant(
    State(state): State<AppState>,
    Path(variant_id): Path<Uuid>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    let variant = state
        .services
        .product_catalog
        .get_variant(variant_id)
        .await
        .map_err(map_service_error)?;

    Ok(success_response(VariantResponse::from(variant)))
}

/// Delete a product variant
#[utoipa::path(
    delete,
    path = "/api/v1/products/variants/:variant_id",
    params(
        ("variant_id" = Uuid, Path, description = "Variant ID")
    ),
    responses(
        (status = 204, description = "Variant deleted"),
        (status = 404, description = "Variant not found", body = crate::errors::ErrorResponse)
    ),
    security(("Bearer" = []), ("ApiKey" = [])),
    tag = "Products"
)]
async fn delete_variant(
    _user: AuthenticatedUser,
    State(state): State<AppState>,
    Path(variant_id): Path<Uuid>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    state
        .services
        .product_catalog
        .delete_variant(variant_id)
        .await
        .map_err(map_service_error)?;

    Ok(no_content_response())
}

/// Update variant price
#[utoipa::path(
    put,
    path = "/api/v1/products/variants/:variant_id/price",
    params(
        ("variant_id" = Uuid, Path, description = "Variant ID")
    ),
    request_body = UpdatePriceRequest,
    responses(
        (status = 200, description = "Variant price updated", body = crate::ApiResponse<MessageResponse>),
        (status = 400, description = "Invalid payload", body = crate::errors::ErrorResponse),
        (status = 404, description = "Variant not found", body = crate::errors::ErrorResponse)
    ),
    security(("Bearer" = []), ("ApiKey" = [])),
    tag = "Products"
)]
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

    Ok(success_response(MessageResponse {
        message: "Price updated successfully".to_string(),
    }))
}

/// Search products
#[utoipa::path(
    get,
    path = "/api/v1/products/search",
    params(ProductSearchParams),
    responses(
        (status = 200, description = "Products search results", body = crate::ApiResponse<ProductSearchResponse>),
        (status = 400, description = "Invalid query parameters", body = crate::errors::ErrorResponse)
    ),
    security(("Bearer" = []), ("ApiKey" = [])),
    tag = "Products"
)]
async fn search_products(
    State(state): State<AppState>,
    Query(params): Query<ProductSearchParams>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    let mut query = ProductSearchQuery {
        search: params.search.clone(),
        is_active: params.is_active,
        limit: params.limit,
        offset: params.offset,
    };

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
#[utoipa::path(
    get,
    path = "/api/v1/products",
    params(PaginationParams),
    responses(
        (status = 200, description = "Products retrieved", body = crate::ApiResponse<PaginatedResponse<ProductResponse>>),
        (status = 400, description = "Invalid query parameters", body = crate::errors::ErrorResponse),
        (status = 401, description = "Unauthorized", body = crate::errors::ErrorResponse)
    ),
    security(("Bearer" = []), ("ApiKey" = [])),
    tag = "Products"
)]
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

#[derive(Debug, Deserialize, Validate, ToSchema)]
#[schema(example = json!({
    "name": "Wireless Bluetooth Headphones",
    "sku": "WBH-BLK-001",
    "description": "Premium over-ear wireless headphones with active noise cancellation, 30-hour battery life, and premium sound quality.",
    "price": "149.99",
    "currency": "USD",
    "is_active": true,
    "is_digital": false,
    "image_url": "https://cdn.example.com/products/wbh-blk-001.jpg",
    "brand": "AudioTech",
    "manufacturer": "AudioTech Inc.",
    "weight_kg": "0.35",
    "dimensions_cm": "20x18x8",
    "tags": "electronics,audio,headphones,wireless,bluetooth",
    "cost_price": "75.00",
    "msrp": "199.99",
    "tax_rate": "0.08",
    "meta_title": "Wireless Bluetooth Headphones | AudioTech",
    "meta_description": "Shop premium wireless Bluetooth headphones with ANC and 30-hour battery life.",
    "reorder_point": 25
}))]
pub struct CreateProductRequest {
    /// Product display name
    #[validate(length(min = 1))]
    #[schema(example = "Wireless Bluetooth Headphones")]
    pub name: String,
    /// Stock keeping unit (unique identifier)
    #[serde(alias = "slug")]
    #[validate(length(min = 1))]
    #[schema(example = "WBH-BLK-001")]
    pub sku: String,
    /// Product description (max 2000 characters)
    #[serde(default)]
    #[validate(length(max = 2000))]
    #[schema(example = "Premium over-ear wireless headphones with active noise cancellation.")]
    pub description: Option<String>,
    /// Sale price
    #[serde(default)]
    #[schema(example = "149.99")]
    pub price: Option<Decimal>,
    /// Currency code (ISO 4217, 3 characters)
    #[serde(default)]
    #[validate(length(equal = 3))]
    #[schema(example = "USD")]
    pub currency: Option<String>,
    /// Whether the product is available for purchase
    #[serde(default)]
    #[schema(example = true)]
    pub is_active: Option<bool>,
    /// Whether this is a digital/downloadable product
    #[serde(default)]
    #[schema(example = false)]
    pub is_digital: Option<bool>,
    /// Main product image URL
    #[serde(default)]
    #[validate(url)]
    #[schema(example = "https://cdn.example.com/products/wbh-blk-001.jpg")]
    pub image_url: Option<String>,
    /// Brand name
    #[serde(default)]
    #[schema(example = "AudioTech")]
    pub brand: Option<String>,
    /// Manufacturer name
    #[serde(default)]
    #[schema(example = "AudioTech Inc.")]
    pub manufacturer: Option<String>,
    /// Product weight in kilograms
    #[serde(default)]
    #[schema(example = "0.35")]
    pub weight_kg: Option<Decimal>,
    /// Dimensions in centimeters (LxWxH format)
    #[serde(default)]
    #[schema(example = "20x18x8")]
    pub dimensions_cm: Option<String>,
    /// Comma-separated tags for categorization
    #[serde(default)]
    #[schema(example = "electronics,audio,headphones,wireless")]
    pub tags: Option<String>,
    /// Cost price (for profit calculation)
    #[serde(default)]
    #[schema(example = "75.00")]
    pub cost_price: Option<Decimal>,
    /// Manufacturer's suggested retail price
    #[serde(default)]
    #[schema(example = "199.99")]
    pub msrp: Option<Decimal>,
    /// Tax rate as decimal
    #[serde(default)]
    #[schema(example = "0.08")]
    pub tax_rate: Option<Decimal>,
    /// SEO meta title
    #[serde(default)]
    #[schema(example = "Wireless Bluetooth Headphones | AudioTech")]
    pub meta_title: Option<String>,
    /// SEO meta description
    #[serde(default)]
    #[schema(example = "Shop premium wireless Bluetooth headphones with ANC.")]
    pub meta_description: Option<String>,
    /// Inventory reorder point threshold
    #[serde(default)]
    #[schema(example = 25)]
    pub reorder_point: Option<i32>,
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
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

#[derive(Debug, Serialize, ToSchema)]
#[schema(example = json!({
    "id": "550e8400-e29b-41d4-a716-446655440000",
    "name": "Wireless Bluetooth Headphones",
    "sku": "WBH-BLK-001",
    "description": "Premium over-ear wireless headphones with active noise cancellation.",
    "price": "149.99",
    "currency": "USD",
    "is_active": true,
    "is_digital": false,
    "image_url": "https://cdn.example.com/products/wbh-blk-001.jpg",
    "brand": "AudioTech",
    "manufacturer": "AudioTech Inc.",
    "weight_kg": "0.35",
    "dimensions_cm": "20x18x8",
    "tags": "electronics,audio,headphones,wireless",
    "cost_price": "75.00",
    "msrp": "199.99",
    "tax_rate": "0.08",
    "meta_title": "Wireless Bluetooth Headphones | AudioTech",
    "meta_description": "Shop premium wireless Bluetooth headphones.",
    "reorder_point": 25,
    "created_at": "2024-12-09T10:30:00Z",
    "updated_at": "2024-12-09T14:45:00Z"
}))]
pub struct ProductResponse {
    /// Product UUID
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub id: Uuid,
    /// Product display name
    #[schema(example = "Wireless Bluetooth Headphones")]
    pub name: String,
    /// Stock keeping unit
    #[schema(example = "WBH-BLK-001")]
    pub sku: String,
    /// Product description
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "Premium over-ear wireless headphones with active noise cancellation.")]
    pub description: Option<String>,
    /// Sale price
    #[schema(example = "149.99")]
    pub price: Decimal,
    /// Currency code (ISO 4217)
    #[schema(example = "USD")]
    pub currency: String,
    /// Whether product is available for purchase
    #[schema(example = true)]
    pub is_active: bool,
    /// Whether this is a digital/downloadable product
    #[schema(example = false)]
    pub is_digital: bool,
    /// Main product image URL
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "https://cdn.example.com/products/wbh-blk-001.jpg")]
    pub image_url: Option<String>,
    /// Brand name
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "AudioTech")]
    pub brand: Option<String>,
    /// Manufacturer name
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "AudioTech Inc.")]
    pub manufacturer: Option<String>,
    /// Product weight in kilograms
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "0.35")]
    pub weight_kg: Option<Decimal>,
    /// Dimensions in centimeters
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "20x18x8")]
    pub dimensions_cm: Option<String>,
    /// Comma-separated tags
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "electronics,audio,headphones")]
    pub tags: Option<String>,
    /// Cost price
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "75.00")]
    pub cost_price: Option<Decimal>,
    /// MSRP
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "199.99")]
    pub msrp: Option<Decimal>,
    /// Tax rate
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "0.08")]
    pub tax_rate: Option<Decimal>,
    /// SEO meta title
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "Wireless Bluetooth Headphones | AudioTech")]
    pub meta_title: Option<String>,
    /// SEO meta description
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "Shop premium wireless Bluetooth headphones.")]
    pub meta_description: Option<String>,
    /// Reorder point threshold
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = 25)]
    pub reorder_point: Option<i32>,
    /// Creation timestamp
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// Last update timestamp
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

#[derive(Debug, Serialize, ToSchema)]
pub struct ProductSearchResponse {
    pub products: Vec<ProductResponse>,
    pub total: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offset: Option<u64>,
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ProductSearchParams {
    #[serde(default)]
    pub search: Option<String>,
    #[serde(default)]
    pub is_active: Option<bool>,
    #[serde(default)]
    pub limit: Option<u64>,
    #[serde(default)]
    pub offset: Option<u64>,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(example = json!({
    "id": "660e8400-e29b-41d4-a716-446655440001",
    "product_id": "550e8400-e29b-41d4-a716-446655440000",
    "sku": "WBH-BLK-001-L",
    "name": "Wireless Bluetooth Headphones - Large",
    "price": "149.99",
    "compare_at_price": "199.99",
    "cost": "75.00",
    "weight": 0.35,
    "dimensions": {
        "length": 20.0,
        "width": 18.0,
        "height": 8.0
    },
    "options": {"size": "Large", "color": "Black"},
    "inventory_tracking": true,
    "position": 1,
    "created_at": "2024-12-09T10:30:00Z",
    "updated_at": "2024-12-09T14:45:00Z"
}))]
pub struct VariantResponse {
    /// Variant UUID
    #[schema(example = "660e8400-e29b-41d4-a716-446655440001")]
    pub id: Uuid,
    /// Parent product UUID
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub product_id: Uuid,
    /// Variant SKU
    #[schema(example = "WBH-BLK-001-L")]
    pub sku: String,
    /// Variant display name
    #[schema(example = "Wireless Bluetooth Headphones - Large")]
    pub name: String,
    /// Sale price
    #[schema(example = "149.99")]
    pub price: Decimal,
    /// Original/compare-at price (for showing discounts)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "199.99")]
    pub compare_at_price: Option<Decimal>,
    /// Cost price
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "75.00")]
    pub cost: Option<Decimal>,
    /// Weight in kilograms
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = 0.35)]
    pub weight: Option<f64>,
    /// Dimensions (length, width, height in cm)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dimensions: Option<product_variant::Dimensions>,
    /// Variant options (e.g., size, color)
    pub options: Value,
    /// Whether inventory is tracked for this variant
    #[schema(example = true)]
    pub inventory_tracking: bool,
    /// Display position (for sorting)
    #[schema(example = 1)]
    pub position: i32,
    /// Creation timestamp
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// Last update timestamp
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

impl From<product_variant::Model> for VariantResponse {
    fn from(model: product_variant::Model) -> Self {
        let dimensions = model
            .dimensions
            .as_ref()
            .and_then(|json| serde_json::from_value(json.clone()).ok());

        Self {
            id: model.id,
            product_id: model.product_id,
            sku: model.sku,
            name: model.name,
            price: model.price,
            compare_at_price: model.compare_at_price,
            cost: model.cost,
            weight: model.weight,
            dimensions,
            options: model.options.clone(),
            inventory_tracking: model.inventory_tracking,
            position: model.position,
            created_at: model.created_at,
            updated_at: model.updated_at,
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct MessageResponse {
    pub message: String,
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
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

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct UpdatePriceRequest {
    #[validate(custom = "validate_decimal_min_zero")]
    pub price: Decimal,
}
