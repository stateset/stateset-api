use crate::auth::{AuthRouterExt, AuthenticatedUser};
use crate::entities::commerce::{cart_item, CartModel, CartStatus};
use crate::handlers::common::{
    created_response, map_service_error, no_content_response, success_response, validate_input,
    PaginatedResponse, PaginationParams,
};
use crate::{
    errors::ApiError,
    services::commerce::cart_service::{AddToCartInput, CartWithItems, CreateCartInput},
    AppState,
};
use axum::{
    extract::{Json, Path, Query, State},
    routing::{delete, get, post, put},
    Router,
};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;
use validator::Validate;

/// Creates the router for cart endpoints
pub fn carts_routes() -> Router<AppState> {
    Router::new()
        .route("/", get(list_carts))
        .route("/", post(create_cart))
        .route("/{id}", get(get_cart))
        .route("/{id}", delete(delete_cart))
        .route("/{id}/items", post(add_to_cart))
        .route("/{id}/items/{item_id}", put(update_cart_item))
        .route("/{id}/items/{item_id}", delete(remove_cart_item))
        .route("/{id}/clear", post(clear_cart))
        .with_auth()
}

/// List carts for the authenticated customer
#[utoipa::path(
    get,
    path = "/api/v1/carts",
    params(PaginationParams),
    responses(
        (status = 200, description = "Carts listed", body = crate::ApiResponse<PaginatedResponse<CartResponse>>),
        (status = 401, description = "Unauthorized", body = crate::errors::ErrorResponse)
    ),
    security(("Bearer" = []), ("ApiKey" = [])),
    tag = "Carts"
)]
async fn list_carts(
    user: AuthenticatedUser,
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

    let user_id = parse_user_id(&user)?;
    let (carts, total) = state
        .services
        .cart
        .list_carts_for_customer(user_id, params.page, params.per_page)
        .await
        .map_err(map_service_error)?;

    let data: Vec<CartResponse> = carts.into_iter().map(CartResponse::from_model).collect();

    Ok(success_response(PaginatedResponse::new(
        data,
        params.page,
        params.per_page,
        total,
    )))
}

/// Create a new cart
#[utoipa::path(
    post,
    path = "/api/v1/carts",
    request_body = CreateCartRequest,
    responses(
        (status = 201, description = "Cart created", body = crate::ApiResponse<CartResponse>),
        (status = 401, description = "Unauthorized", body = crate::errors::ErrorResponse),
        (status = 403, description = "Forbidden", body = crate::errors::ErrorResponse)
    ),
    security(("Bearer" = []), ("ApiKey" = [])),
    tag = "Carts"
)]
async fn create_cart(
    user: AuthenticatedUser,
    State(state): State<AppState>,
    Json(payload): Json<CreateCartRequest>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    let user_id = parse_user_id(&user)?;

    if let Some(customer_id) = payload.customer_id {
        if customer_id != user_id {
            return Err(ApiError::Unauthorized);
        }
    }

    let input = CreateCartInput {
        session_id: payload.session_id,
        customer_id: Some(user_id),
        currency: payload.currency,
        metadata: payload.metadata,
    };

    let cart = state
        .services
        .cart
        .create_cart(input)
        .await
        .map_err(map_service_error)?;

    Ok(created_response(CartResponse::from_model(cart)))
}

/// Get cart with items
#[utoipa::path(
    get,
    path = "/api/v1/carts/{id}",
    params(
        ("id" = Uuid, Path, description = "Cart ID")
    ),
    responses(
        (status = 200, description = "Cart retrieved", body = crate::ApiResponse<CartDetailedResponse>),
        (status = 404, description = "Cart not found", body = crate::errors::ErrorResponse)
    ),
    security(("Bearer" = []), ("ApiKey" = [])),
    tag = "Carts"
)]
async fn get_cart(
    user: AuthenticatedUser,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    let cart_with_items = state
        .services
        .cart
        .get_cart(id)
        .await
        .map_err(map_service_error)?;

    verify_cart_owner(cart_with_items.cart.customer_id, parse_user_id(&user)?)?;

    Ok(success_response(CartDetailedResponse::from_model(
        cart_with_items,
    )))
}

/// Delete (abandon) a cart
#[utoipa::path(
    delete,
    path = "/api/v1/carts/{id}",
    params(("id" = Uuid, Path, description = "Cart ID")),
    responses(
        (status = 200, description = "Cart abandoned", body = crate::ApiResponse<CartResponse>),
        (status = 404, description = "Cart not found", body = crate::errors::ErrorResponse)
    ),
    security(("Bearer" = []), ("ApiKey" = [])),
    tag = "Carts"
)]
async fn delete_cart(
    user: AuthenticatedUser,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    ensure_cart_access(&state, id, parse_user_id(&user)?).await?;

    let cart = state
        .services
        .cart
        .abandon_cart(id)
        .await
        .map_err(map_service_error)?;

    Ok(success_response(CartResponse::from_model(cart)))
}

/// Add item to cart
#[utoipa::path(
    post,
    path = "/api/v1/carts/{id}/items",
    params(
        ("id" = Uuid, Path, description = "Cart ID")
    ),
    request_body = AddItemRequest,
    responses(
        (status = 200, description = "Item added", body = crate::ApiResponse<CartResponse>),
        (status = 404, description = "Cart not found", body = crate::errors::ErrorResponse)
    ),
    security(("Bearer" = []), ("ApiKey" = [])),
    tag = "Carts"
)]
async fn add_to_cart(
    user: AuthenticatedUser,
    State(state): State<AppState>,
    Path(cart_id): Path<Uuid>,
    Json(payload): Json<AddItemRequest>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    validate_input(&payload)?;

    ensure_cart_access(&state, cart_id, parse_user_id(&user)?).await?;

    let input = AddToCartInput {
        variant_id: payload.variant_id,
        quantity: payload.quantity,
    };

    let cart = state
        .services
        .cart
        .add_item(cart_id, input)
        .await
        .map_err(map_service_error)?;

    Ok(success_response(CartResponse::from_model(cart)))
}

/// Update cart item quantity
#[utoipa::path(
    put,
    path = "/api/v1/carts/{id}/items/{item_id}",
    params(
        ("id" = Uuid, Path, description = "Cart ID"),
        ("item_id" = Uuid, Path, description = "Cart item ID")
    ),
    request_body = UpdateQuantityRequest,
    responses(
        (status = 200, description = "Cart item updated", body = crate::ApiResponse<CartResponse>),
        (status = 404, description = "Cart or item not found", body = crate::errors::ErrorResponse)
    ),
    security(("Bearer" = []), ("ApiKey" = [])),
    tag = "Carts"
)]
async fn update_cart_item(
    user: AuthenticatedUser,
    State(state): State<AppState>,
    Path((cart_id, item_id)): Path<(Uuid, Uuid)>,
    Json(payload): Json<UpdateQuantityRequest>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    validate_input(&payload)?;

    ensure_cart_access(&state, cart_id, parse_user_id(&user)?).await?;

    let cart = state
        .services
        .cart
        .update_item_quantity(cart_id, item_id, payload.quantity)
        .await
        .map_err(map_service_error)?;

    Ok(success_response(CartResponse::from_model(cart)))
}

/// Remove item from cart
#[utoipa::path(
    delete,
    path = "/api/v1/carts/{id}/items/{item_id}",
    params(
        ("id" = Uuid, Path, description = "Cart ID"),
        ("item_id" = Uuid, Path, description = "Cart item ID")
    ),
    responses(
        (status = 204, description = "Item removed"),
        (status = 404, description = "Cart or item not found", body = crate::errors::ErrorResponse)
    ),
    security(("Bearer" = []), ("ApiKey" = [])),
    tag = "Carts"
)]
async fn remove_cart_item(
    user: AuthenticatedUser,
    State(state): State<AppState>,
    Path((cart_id, item_id)): Path<(Uuid, Uuid)>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    ensure_cart_access(&state, cart_id, parse_user_id(&user)?).await?;

    state
        .services
        .cart
        .update_item_quantity(cart_id, item_id, 0)
        .await
        .map_err(map_service_error)?;

    Ok(no_content_response())
}

/// Clear all items from cart
#[utoipa::path(
    post,
    path = "/api/v1/carts/{id}/clear",
    params(
        ("id" = Uuid, Path, description = "Cart ID")
    ),
    responses(
        (status = 200, description = "Cart cleared", body = crate::ApiResponse<CartMessageResponse>),
        (status = 404, description = "Cart not found", body = crate::errors::ErrorResponse)
    ),
    security(("Bearer" = []), ("ApiKey" = [])),
    tag = "Carts"
)]
async fn clear_cart(
    user: AuthenticatedUser,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    ensure_cart_access(&state, id, parse_user_id(&user)?).await?;

    state
        .services
        .cart
        .clear_cart(id)
        .await
        .map_err(map_service_error)?;

    Ok(success_response(CartMessageResponse {
        message: "Cart cleared successfully".to_string(),
    }))
}

// Request DTOs

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateCartRequest {
    pub session_id: Option<String>,
    pub customer_id: Option<Uuid>,
    pub currency: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct AddItemRequest {
    pub variant_id: Uuid,
    #[validate(range(min = 1))]
    pub quantity: i32,
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct UpdateQuantityRequest {
    #[validate(range(min = 0))]
    pub quantity: i32,
}

fn parse_user_id(user: &AuthenticatedUser) -> Result<Uuid, ApiError> {
    Uuid::parse_str(&user.user_id).map_err(|_| ApiError::Unauthorized)
}

async fn ensure_cart_access(
    state: &AppState,
    cart_id: Uuid,
    user_id: Uuid,
) -> Result<(), ApiError> {
    let cart = state
        .services
        .cart
        .get_cart_model(cart_id)
        .await
        .map_err(map_service_error)?;

    verify_cart_owner(cart.customer_id, user_id)
}

fn verify_cart_owner(customer_id: Option<Uuid>, user_id: Uuid) -> Result<(), ApiError> {
    match customer_id {
        Some(owner) if owner == user_id => Ok(()),
        _ => Err(ApiError::Unauthorized),
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct CartResponse {
    pub id: Uuid,
    pub currency: String,
    pub subtotal: Decimal,
    pub tax_total: Decimal,
    pub shipping_total: Decimal,
    pub discount_total: Decimal,
    pub total: Decimal,
    pub status: CartStatus,
    pub expires_at: chrono::DateTime<chrono::Utc>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

impl CartResponse {
    fn from_model(model: CartModel) -> Self {
        Self {
            id: model.id,
            currency: model.currency,
            subtotal: model.subtotal,
            tax_total: model.tax_total,
            shipping_total: model.shipping_total,
            discount_total: model.discount_total,
            total: model.total,
            status: model.status,
            expires_at: model.expires_at,
            created_at: model.created_at,
            updated_at: model.updated_at,
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct CartItemResponse {
    pub id: Uuid,
    pub variant_id: Uuid,
    pub quantity: i32,
    pub unit_price: Decimal,
    pub line_total: Decimal,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

impl From<cart_item::Model> for CartItemResponse {
    fn from(model: cart_item::Model) -> Self {
        Self {
            id: model.id,
            variant_id: model.variant_id,
            quantity: model.quantity,
            unit_price: model.unit_price,
            line_total: model.line_total,
            created_at: model.created_at,
            updated_at: model.updated_at,
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct CartDetailedResponse {
    pub cart: CartResponse,
    pub items: Vec<CartItemResponse>,
}

impl CartDetailedResponse {
    fn from_model(model: CartWithItems) -> Self {
        Self {
            cart: CartResponse::from_model(model.cart),
            items: model.items.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct CartMessageResponse {
    pub message: String,
}
