use crate::auth::AuthenticatedUser;
use crate::errors::ServiceError;
use crate::handlers::AppState;
use axum::{
    extract::{Json, Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post, put},
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;
use validator::Validate;

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct RegisterCustomerRequest {
    pub email: String,

    pub first_name: String,

    pub last_name: String,

    pub password: String,
    pub phone: Option<String>,
    pub accepts_marketing: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CustomerLoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct UpdateCustomerRequest {
    pub first_name: Option<String>,

    pub last_name: Option<String>,
    pub phone: Option<String>,
    pub accepts_marketing: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct AddAddressRequest {
    pub first_name: String,

    pub last_name: String,
    pub company: Option<String>,

    pub address_line_1: String,
    pub address_line_2: Option<String>,

    pub city: String,

    pub province: String,

    pub country_code: String,

    pub postal_code: String,
    pub phone: Option<String>,
    pub is_default_shipping: Option<bool>,
    pub is_default_billing: Option<bool>,
}

// Handler functions

/// Register a new customer
async fn register_customer(
    State(state): State<Arc<AppState>>,
    Json(request): Json<RegisterCustomerRequest>,
) -> Result<impl IntoResponse, ServiceError> {
    request.validate()?;

    let input = crate::services::commerce::customer_service::RegisterCustomerInput {
        email: request.email,
        first_name: request.first_name,
        last_name: request.last_name,
        password: request.password,
        phone: request.phone,
        accepts_marketing: request.accepts_marketing.unwrap_or(false),
    };

    let customer = state.services.customer.register_customer(input).await?;
    Ok((StatusCode::CREATED, Json(customer)))
}

/// Customer login
async fn login_customer(
    State(state): State<Arc<AppState>>,
    Json(request): Json<CustomerLoginRequest>,
) -> Result<impl IntoResponse, ServiceError> {
    let credentials = crate::auth::LoginCredentials {
        email: request.email,
        password: request.password,
    };

    let response = state.services.customer.login(credentials).await?;
    Ok(Json(response))
}

/// Get customer profile
async fn get_customer(
    State(state): State<Arc<AppState>>,
    Path(customer_id): Path<Uuid>,
    user: AuthenticatedUser,
) -> Result<impl IntoResponse, ServiceError> {
    // Check if user is requesting their own profile or has admin permissions
    if user.user_id != customer_id.to_string() && !user.has_permission("customers:read") {
        return Err(ServiceError::Forbidden("Access denied".to_string()));
    }

    let customer = state.services.customer.get_customer(customer_id).await?;
    Ok(Json(customer))
}

/// Update customer profile
async fn update_customer(
    State(state): State<Arc<AppState>>,
    Path(customer_id): Path<Uuid>,
    user: AuthenticatedUser,
    Json(request): Json<UpdateCustomerRequest>,
) -> Result<impl IntoResponse, ServiceError> {
    // Check if user is updating their own profile or has admin permissions
    if user.user_id != customer_id.to_string() && !user.has_permission("customers:write") {
        return Err(ServiceError::Forbidden("Access denied".to_string()));
    }

    request.validate()?;

    let input = crate::services::commerce::customer_service::UpdateCustomerInput {
        first_name: request.first_name,
        last_name: request.last_name,
        phone: request.phone,
        accepts_marketing: request.accepts_marketing,
    };

    let customer = state
        .services
        .customer
        .update_customer(customer_id, input)
        .await?;
    Ok(Json(customer))
}

/// Add customer address
async fn add_customer_address(
    State(state): State<Arc<AppState>>,
    Path(customer_id): Path<Uuid>,
    user: AuthenticatedUser,
    Json(request): Json<AddAddressRequest>,
) -> Result<impl IntoResponse, ServiceError> {
    // Check if user is updating their own profile or has admin permissions
    if user.user_id != customer_id.to_string() && !user.has_permission("customers:write") {
        return Err(ServiceError::Forbidden("Access denied".to_string()));
    }

    request.validate()?;

    let input = crate::services::commerce::customer_service::AddAddressInput {
        first_name: request.first_name,
        last_name: request.last_name,
        company: request.company,
        address_line_1: request.address_line_1,
        address_line_2: request.address_line_2,
        city: request.city,
        province: request.province,
        country_code: request.country_code,
        postal_code: request.postal_code,
        phone: request.phone,
        is_default_shipping: request.is_default_shipping,
        is_default_billing: request.is_default_billing,
    };

    let address = state
        .services
        .customer
        .add_address(customer_id, input)
        .await?;
    Ok((StatusCode::CREATED, Json(address)))
}

/// Get customer addresses
async fn get_customer_addresses(
    State(state): State<Arc<AppState>>,
    Path(customer_id): Path<Uuid>,
    user: AuthenticatedUser,
) -> Result<impl IntoResponse, ServiceError> {
    // Check if user is accessing their own profile or has admin permissions
    if user.user_id != customer_id.to_string() && !user.has_permission("customers:read") {
        return Err(ServiceError::Forbidden("Access denied".to_string()));
    }

    let addresses = state.services.customer.get_addresses(customer_id).await?;
    Ok(Json(addresses))
}

/// Customer routes
pub fn customer_routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/register", post(register_customer))
        .route("/login", post(login_customer))
        .route("/:customer_id", get(get_customer))
        .route("/:customer_id", put(update_customer))
        .route("/:customer_id/addresses", post(add_customer_address))
        .route("/:customer_id/addresses", get(get_customer_addresses))
}
