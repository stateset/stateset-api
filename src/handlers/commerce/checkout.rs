use crate::handlers::common::{created_response, map_service_error, success_response, validate_input};
use crate::{
    auth::AuthenticatedUser,
    errors::ApiError,
    services::commerce::checkout_service::{
        Address, CheckoutSession, CustomerInfoInput, PaymentInfo, ShippingMethod,
    },
    AppState,
};
use axum::{
    extract::{Json, Path, State},
    response::IntoResponse,
    routing::{get, post, put},
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;
use validator::Validate;

/// Creates the router for checkout endpoints
pub fn checkout_routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", post(start_checkout))
        .route("/{session_id}", get(get_checkout_session))
        .route("/{session_id}/customer", put(set_customer_info))
        .route("/{session_id}/shipping-address", put(set_shipping_address))
        .route("/{session_id}/shipping-method", put(set_shipping_method))
        .route("/{session_id}/complete", post(complete_checkout))
}

/// Start checkout from cart
async fn start_checkout(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<StartCheckoutRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let session = state
        .services
        .checkout
        .start_checkout(payload.cart_id)
        .await
        .map_err(map_service_error)?;

    Ok(created_response(CheckoutSessionResponse::from(session)))
}

/// Get checkout session
async fn get_checkout_session(
    State(_state): State<Arc<AppState>>,
    Path(_session_id): Path<Uuid>,
) -> Result<axum::response::Response, ApiError> {
    // For now, we'll need to store sessions in memory or cache
    // This is a simplified version
    Err(ApiError::NotFound("Session storage not implemented yet".to_string()))
}

/// Set customer info
async fn set_customer_info(
    State(_state): State<Arc<AppState>>,
    Path(_session_id): Path<Uuid>,
    Json(payload): Json<CustomerInfoRequest>,
) -> Result<impl IntoResponse, ApiError> {
    validate_input(&payload)?;

    // This would retrieve and update the session
    // For now, returning success
    Ok(success_response(serde_json::json!({
        "message": "Customer info updated"
    })))
}

/// Set shipping address
async fn set_shipping_address(
    State(_state): State<Arc<AppState>>,
    Path(_session_id): Path<Uuid>,
    Json(payload): Json<AddressRequest>,
) -> Result<impl IntoResponse, ApiError> {
    validate_input(&payload)?;

    let _address = Address {
        first_name: payload.first_name,
        last_name: payload.last_name,
        company: payload.company,
        address_line_1: payload.address_line_1,
        address_line_2: payload.address_line_2,
        city: payload.city,
        province: payload.province,
        country_code: payload.country_code,
        postal_code: payload.postal_code,
        phone: payload.phone,
    };

    // This would update the session
    Ok(success_response(serde_json::json!({
        "message": "Shipping address updated"
    })))
}

/// Set shipping method
async fn set_shipping_method(
    State(_state): State<Arc<AppState>>,
    Path(_session_id): Path<Uuid>,
    Json(payload): Json<ShippingMethodRequest>,
) -> Result<impl IntoResponse, ApiError> {
    // This would update session and calculate shipping
    let mock_rate = crate::services::commerce::checkout_service::ShippingRate {
        method: payload.method.clone(),
        amount: rust_decimal::Decimal::from(10),
        estimated_days: 3,
    };

    Ok(success_response(mock_rate))
}

/// Complete checkout
async fn complete_checkout(
    State(state): State<Arc<AppState>>,
    Path(_session_id): Path<Uuid>,
    Json(payload): Json<CompleteCheckoutRequest>,
) -> Result<impl IntoResponse, ApiError> {
    validate_input(&payload)?;

    // TODO: Re-implement session management
    // let session = state
    //     .services
    //     .session_manager
    //     .get_session(&payload.payment_token)
    //     .await
    //     .map_err(map_service_error)?;

    // if session.cart.items.is_empty() {
    //     return Err(ApiError::BadRequest {
    //         message: "Cannot proceed to checkout with an empty cart".to_string(),
    //         error_code: Some("EMPTY_CART".to_string()),
    //     });
    // }

    // // Ensure all items are in stock
    // for item in &session.cart.items {
    //     let stock = get_stock_level(item.product_id, &state.db).await?;
    //     if item.quantity > stock {
    //         return Err(ApiError::BadRequest {
    //             message: format!(
    //                 "Insufficient stock for product {}: requested {}, available {}",
    //                 item.product_id, item.quantity, stock
    //             ),
    //             error_code: Some("INSUFFICIENT_STOCK".to_string()),
    //         });
    //     }
    // }

    // // Create an order
    // let order_id = create_order_from_cart(&session, &state.db).await?;

    // // Clear the cart
    // session.cart.items.clear();
    // state
    //     .services
    //     .session_manager
    //     .save_session(&session)
    //     .await
    //     .map_err(|e| {
    //         error!("Failed to save session: {}", e);
    //         ApiError::InternalServerError {
    //             message: "Failed to update session".to_string(),
    //         }
    //     })?;

    Ok(created_response(serde_json::json!({ "order_id": Uuid::new_v4() })))
}

async fn get_stock_level(
    _product_id: Uuid,
    _db: &Arc<crate::db::DbPool>,
) -> Result<i32, ApiError> {
    // Dummy implementation
    Ok(100)
}

async fn create_order_from_cart(
    _session: &(),
    _db: &Arc<crate::db::DbPool>,
) -> Result<Uuid, ApiError> {
    // Dummy implementation
    Ok(Uuid::new_v4())
}

// Request/Response DTOs

#[derive(Debug, Deserialize)]
pub struct StartCheckoutRequest {
    pub cart_id: Uuid,
}

#[derive(Debug, Serialize)]
pub struct CheckoutSessionResponse {
    pub id: Uuid,
    pub cart_id: Uuid,
    pub step: String,
    pub subtotal: rust_decimal::Decimal,
    pub total: rust_decimal::Decimal,
    pub currency: String,
}

impl From<CheckoutSession> for CheckoutSessionResponse {
    fn from(session: CheckoutSession) -> Self {
        Self {
            id: session.id,
            cart_id: session.cart_id,
            step: format!("{:?}", session.step),
            subtotal: session.cart.subtotal,
            total: session.cart.total,
            currency: session.cart.currency,
        }
    }
}

#[derive(Debug, Deserialize, Validate)]
pub struct CustomerInfoRequest {
    #[validate(email)]
    pub email: String,
    pub subscribe_newsletter: bool,
}

#[derive(Debug, Deserialize, Validate)]
pub struct AddressRequest {
    #[validate(length(min = 1))]
    pub first_name: String,
    #[validate(length(min = 1))]
    pub last_name: String,
    pub company: Option<String>,
    #[validate(length(min = 1))]
    pub address_line_1: String,
    pub address_line_2: Option<String>,
    #[validate(length(min = 1))]
    pub city: String,
    #[validate(length(min = 1))]
    pub province: String,
    #[validate(length(equal = 2))]
    pub country_code: String,
    #[validate(length(min = 1))]
    pub postal_code: String,
    pub phone: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ShippingMethodRequest {
    pub method: ShippingMethod,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CompleteCheckoutRequest {
    pub payment_method: crate::services::commerce::checkout_service::PaymentMethod,
    #[validate(length(min = 1))]
    pub payment_token: String,
}
