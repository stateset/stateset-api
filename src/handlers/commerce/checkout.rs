use crate::handlers::common::{
    created_response, map_service_error, success_response, validate_input,
};
use crate::{
    errors::{ApiError, ServiceError},
    services::commerce::checkout_service::{
        Address, CheckoutSession, CustomerInfoInput, PaymentInfo, PaymentMethod, ShippingMethod,
        ShippingRate,
    },
    AppState,
};
use axum::{
    extract::{Json, Path, State},
    response::IntoResponse,
    routing::{get, post, put},
    Router,
};
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use serde_json::{from_str, to_string};
use std::fmt;
use uuid::Uuid;
use validator::Validate;

const CHECKOUT_SESSION_TTL_SECS: usize = 60 * 30;

fn checkout_session_key(id: &Uuid) -> String {
    format!("checkout_session:{}", id)
}

fn cache_error(context: &str, err: impl fmt::Display) -> ApiError {
    ApiError::ServiceError(ServiceError::CacheError(format!("{}: {}", context, err)))
}

fn serialization_error(context: &str, err: impl fmt::Display) -> ApiError {
    ApiError::ServiceError(ServiceError::SerializationError(format!(
        "{}: {}",
        context, err
    )))
}

async fn redis_connection(state: &AppState) -> Result<redis::aio::Connection, ApiError> {
    state
        .redis
        .get_async_connection()
        .await
        .map_err(|e| cache_error("failed to acquire redis connection", e))
}

async fn persist_session(state: &AppState, session: &CheckoutSession) -> Result<(), ApiError> {
    let mut conn = redis_connection(state).await?;
    let payload = to_string(session)
        .map_err(|e| serialization_error("failed to serialize checkout session", e))?;
    conn.set_ex::<_, _, ()>(
        checkout_session_key(&session.id),
        payload,
        CHECKOUT_SESSION_TTL_SECS,
    )
    .await
    .map_err(|e| cache_error("failed to store checkout session", e))?;
    Ok(())
}

async fn load_session(state: &AppState, session_id: &Uuid) -> Result<CheckoutSession, ApiError> {
    let key = checkout_session_key(session_id);
    let mut conn = redis_connection(state).await?;
    let payload: Option<String> = conn
        .get(&key)
        .await
        .map_err(|e| cache_error("failed to load checkout session", e))?;

    let payload = match payload {
        Some(value) => value,
        None => {
            return Err(ApiError::NotFound(format!(
                "Checkout session {} not found",
                session_id
            )))
        }
    };

    let session: CheckoutSession = from_str(&payload)
        .map_err(|e| serialization_error("failed to deserialize checkout session", e))?;

    let _: () = conn
        .expire(&key, CHECKOUT_SESSION_TTL_SECS)
        .await
        .map_err(|e| cache_error("failed to refresh checkout session TTL", e))?;

    Ok(session)
}

async fn remove_session(state: &AppState, session_id: &Uuid) -> Result<(), ApiError> {
    let mut conn = redis_connection(state).await?;
    let _: () = conn
        .del(checkout_session_key(session_id))
        .await
        .map_err(|e| cache_error("failed to delete checkout session", e))?;
    Ok(())
}

/// Creates the router for checkout endpoints
pub fn checkout_routes() -> Router<AppState> {
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
    State(state): State<AppState>,
    Json(payload): Json<StartCheckoutRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let session = state
        .services
        .checkout
        .start_checkout(payload.cart_id)
        .await
        .map_err(map_service_error)?;

    persist_session(&state, &session).await?;

    Ok(created_response(CheckoutSessionResponse::from(session)))
}

/// Get checkout session
async fn get_checkout_session(
    State(state): State<AppState>,
    Path(session_id): Path<Uuid>,
) -> Result<impl IntoResponse, ApiError> {
    let session = load_session(&state, &session_id).await?;

    Ok(success_response(CheckoutSessionResponse::from(session)))
}

/// Set customer info
async fn set_customer_info(
    State(state): State<AppState>,
    Path(session_id): Path<Uuid>,
    Json(payload): Json<CustomerInfoRequest>,
) -> Result<impl IntoResponse, ApiError> {
    validate_input(&payload)?;

    let mut session = load_session(&state, &session_id).await?;

    let input = CustomerInfoInput {
        email: payload.email.clone(),
        subscribe_newsletter: payload.subscribe_newsletter,
    };

    state
        .services
        .checkout
        .set_customer_info(&mut session, input)
        .await
        .map_err(map_service_error)?;

    persist_session(&state, &session).await?;

    Ok(success_response(CheckoutSessionResponse::from(session)))
}

/// Set shipping address
async fn set_shipping_address(
    State(state): State<AppState>,
    Path(session_id): Path<Uuid>,
    Json(payload): Json<AddressRequest>,
) -> Result<impl IntoResponse, ApiError> {
    validate_input(&payload)?;

    let mut session = load_session(&state, &session_id).await?;

    let address = Address {
        first_name: payload.first_name.clone(),
        last_name: payload.last_name.clone(),
        company: payload.company.clone(),
        address_line_1: payload.address_line_1.clone(),
        address_line_2: payload.address_line_2.clone(),
        city: payload.city.clone(),
        province: payload.province.clone(),
        country_code: payload.country_code.clone(),
        postal_code: payload.postal_code.clone(),
        phone: payload.phone.clone(),
    };

    state
        .services
        .checkout
        .set_shipping_address(&mut session, address)
        .await
        .map_err(map_service_error)?;

    persist_session(&state, &session).await?;

    Ok(success_response(CheckoutSessionResponse::from(session)))
}

/// Set shipping method
async fn set_shipping_method(
    State(state): State<AppState>,
    Path(session_id): Path<Uuid>,
    Json(payload): Json<ShippingMethodRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let mut session = load_session(&state, &session_id).await?;

    let rate: ShippingRate = state
        .services
        .checkout
        .set_shipping_method(&mut session, payload.method.clone())
        .await
        .map_err(map_service_error)?;

    persist_session(&state, &session).await?;

    Ok(success_response(rate))
}

/// Complete checkout
async fn complete_checkout(
    State(state): State<AppState>,
    Path(session_id): Path<Uuid>,
    Json(payload): Json<CompleteCheckoutRequest>,
) -> Result<impl IntoResponse, ApiError> {
    validate_input(&payload)?;

    let session = load_session(&state, &session_id).await?;

    let payment_info = PaymentInfo {
        method: payload.payment_method.clone(),
        token: payload.payment_token.clone(),
    };

    let order = state
        .services
        .checkout
        .complete_checkout(session, payment_info)
        .await
        .map_err(map_service_error)?;

    remove_session(&state, &session_id).await?;

    Ok(created_response(
        serde_json::json!({ "order_id": order.id }),
    ))
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
    pub customer_email: Option<String>,
    pub shipping_address: Option<Address>,
    pub billing_address: Option<Address>,
    pub shipping_method: Option<ShippingMethod>,
    pub payment_method: Option<PaymentMethod>,
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
            customer_email: session.customer_email,
            shipping_address: session.shipping_address,
            billing_address: session.billing_address,
            shipping_method: session.shipping_method,
            payment_method: session.payment_method,
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
