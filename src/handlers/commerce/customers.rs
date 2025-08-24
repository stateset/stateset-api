use crate::handlers::common::{created_response, map_service_error, success_response, validate_input};
use crate::{
    auth::{AuthenticatedUser, LoginCredentials},
    errors::ApiError,
    services::commerce::customer_service::{
        AddAddressInput, RegisterCustomerInput, UpdateCustomerInput,
    },
    AppState,
};
use axum::{
    extract::{Json, Path, State},
    routing::{get, post, put},
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;
use validator::Validate;

/// Creates the router for customer endpoints
pub fn customers_routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/register", post(register_customer))
        .route("/login", post(login_customer))
        .route("/me", get(get_current_customer))
        .route("/me", put(update_current_customer))
        .route("/me/addresses", get(get_customer_addresses))
        .route("/me/addresses", post(add_customer_address))
}

/// Register new customer
async fn register_customer(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<RegisterRequest>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    validate_input(&payload)?;

    let input = RegisterCustomerInput {
        email: payload.email,
        password: payload.password,
        first_name: payload.first_name,
        last_name: payload.last_name,
        phone: payload.phone,
        accepts_marketing: payload.accepts_marketing,
    };

    let customer = state
        .services
        .customer
        .register_customer(input)
        .await
        .map_err(map_service_error)?;

    Ok(created_response(crate::services::commerce::customer_service::CustomerResponse::from(customer)))
}

/// Login customer
async fn login_customer(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<LoginRequest>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    validate_input(&payload)?;

    let credentials = LoginCredentials {
        email: payload.email,
        password: payload.password,
    };

    let response = state
        .services
        .customer
        .login(credentials)
        .await
        .map_err(map_service_error)?;

    Ok(success_response(response))
}

/// Get current customer profile
async fn get_current_customer(
    user: AuthenticatedUser,
    State(state): State<Arc<AppState>>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    let customer_id = Uuid::parse_str(&user.user_id)
        .map_err(|_| ApiError::ValidationError("Invalid user ID".to_string()))?;

    let customer = state
        .services
        .customer
        .get_customer(customer_id)
        .await
        .map_err(map_service_error)?;

    Ok(success_response(crate::services::commerce::customer_service::CustomerResponse::from(customer)))
}

/// Update current customer
async fn update_current_customer(
    user: AuthenticatedUser,
    State(state): State<Arc<AppState>>,
    Json(payload): Json<UpdateCustomerRequest>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    let customer_id = Uuid::parse_str(&user.user_id)
        .map_err(|_| ApiError::ValidationError("Invalid user ID".to_string()))?;

    let input = UpdateCustomerInput {
        first_name: payload.first_name,
        last_name: payload.last_name,
        phone: payload.phone,
        accepts_marketing: payload.accepts_marketing,
    };

    let customer = state
        .services
        .customer
        .update_customer(customer_id, input)
        .await
        .map_err(map_service_error)?;

    Ok(success_response(crate::services::commerce::customer_service::CustomerResponse::from(customer)))
}

/// Get customer addresses
async fn get_customer_addresses(
    user: AuthenticatedUser,
    State(state): State<Arc<AppState>>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    let customer_id = Uuid::parse_str(&user.user_id)
        .map_err(|_| ApiError::ValidationError("Invalid user ID".to_string()))?;

    let addresses = state
        .services
        .customer
        .get_addresses(customer_id)
        .await
        .map_err(map_service_error)?;

    Ok(success_response(addresses))
}

/// Add customer address
async fn add_customer_address(
    user: AuthenticatedUser,
    State(state): State<Arc<AppState>>,
    Json(payload): Json<AddAddressRequest>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    validate_input(&payload)?;

    let customer_id = Uuid::parse_str(&user.user_id)
        .map_err(|_| ApiError::ValidationError("Invalid user ID".to_string()))?;

    let input = AddAddressInput {
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
        is_default_shipping: payload.is_default_shipping,
        is_default_billing: payload.is_default_billing,
    };

    let address = state
        .services
        .customer
        .add_address(customer_id, input)
        .await
        .map_err(map_service_error)?;

    Ok(created_response(address))
}

// Request DTOs

#[derive(Debug, Deserialize, Validate)]
pub struct RegisterRequest {
    #[validate(email)]
    pub email: String,
    #[validate(length(min = 8))]
    pub password: String,
    #[validate(length(min = 1))]
    pub first_name: String,
    #[validate(length(min = 1))]
    pub last_name: String,
    pub phone: Option<String>,
    pub accepts_marketing: bool,
}

#[derive(Debug, Deserialize, Validate)]
pub struct LoginRequest {
    #[validate(email)]
    pub email: String,
    #[validate(length(min = 1))]
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateCustomerRequest {
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub phone: Option<String>,
    pub accepts_marketing: Option<bool>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct AddAddressRequest {
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
    pub is_default_shipping: Option<bool>,
    pub is_default_billing: Option<bool>,
}
