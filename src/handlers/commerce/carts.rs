use crate::handlers::common::{created_response, map_service_error, no_content_response, success_response, validate_input};
use crate::{
    auth::AuthenticatedUser,
    errors::ApiError,
    services::commerce::cart_service::{AddToCartInput, CreateCartInput},
    AppState,
};
use axum::{
    extract::{Json, Path, State},
    routing::{delete, get, post, put},
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;
use validator::Validate;

/// Creates the router for cart endpoints
pub fn carts_routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", post(create_cart))
        .route("/{id}", get(get_cart))
        .route("/{id}/items", post(add_to_cart))
        .route("/{id}/items/{item_id}", put(update_cart_item))
        .route("/{id}/items/{item_id}", delete(remove_cart_item))
        .route("/{id}/clear", post(clear_cart))
}

/// Create a new cart
async fn create_cart(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<CreateCartRequest>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    let input = CreateCartInput {
        session_id: payload.session_id,
        customer_id: payload.customer_id,
        currency: payload.currency,
        metadata: payload.metadata,
    };

    let cart = state
        .services
        .cart
        .create_cart(input)
        .await
        .map_err(map_service_error)?;

    Ok(created_response(cart))
}

/// Get cart with items
async fn get_cart(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    let cart_with_items = state
        .services
        .cart
        .get_cart(id)
        .await
        .map_err(map_service_error)?;

    Ok(success_response(cart_with_items))
}

/// Add item to cart
async fn add_to_cart(
    State(state): State<Arc<AppState>>,
    Path(cart_id): Path<Uuid>,
    Json(payload): Json<AddItemRequest>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    validate_input(&payload)?;

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

    Ok(success_response(cart))
}

/// Update cart item quantity
async fn update_cart_item(
    State(state): State<Arc<AppState>>,
    Path((cart_id, item_id)): Path<(Uuid, Uuid)>,
    Json(payload): Json<UpdateQuantityRequest>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    validate_input(&payload)?;

    let cart = state
        .services
        .cart
        .update_item_quantity(cart_id, item_id, payload.quantity)
        .await
        .map_err(map_service_error)?;

    Ok(success_response(cart))
}

/// Remove item from cart
async fn remove_cart_item(
    State(state): State<Arc<AppState>>,
    Path((cart_id, item_id)): Path<(Uuid, Uuid)>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    state
        .services
        .cart
        .update_item_quantity(cart_id, item_id, 0)
        .await
        .map_err(map_service_error)?;

    Ok(no_content_response())
}

/// Clear all items from cart
async fn clear_cart(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    state
        .services
        .cart
        .clear_cart(id)
        .await
        .map_err(map_service_error)?;

    Ok(success_response(serde_json::json!({
        "message": "Cart cleared successfully"
    })))
}

// Request DTOs

#[derive(Debug, Deserialize)]
pub struct CreateCartRequest {
    pub session_id: Option<String>,
    pub customer_id: Option<Uuid>,
    pub currency: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct AddItemRequest {
    pub variant_id: Uuid,
    #[validate(range(min = 1))]
    pub quantity: i32,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateQuantityRequest {
    #[validate(range(min = 0))]
    pub quantity: i32,
} 
