use super::common::PaginationParams;
use crate::auth::AuthenticatedUser;
use crate::db::DbPool;
use crate::errors::ServiceError;
use crate::models::customer::{Entity as CustomerEntity, Model as Customer};
use crate::services::customers::{
    create_customer as create_customer_service, delete_customer as delete_customer_service,
    get_customer as get_customer_service, get_customer_orders as get_customer_orders_service,
    get_customer_returns as get_customer_returns_service, list_customers as list_customers_service,
    search_customers as search_customers_service, update_customer as update_customer_service,
};
use axum::{
    extract::{Json, Path, Query, State},
    response::IntoResponse,
    routing::{delete, get, post, put},
    Router,
};
use serde::Deserialize;
use std::sync::Arc;
use validator::Validate;

#[derive(Deserialize)]
pub struct CustomerSearchParams {
    pub name: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub status: Option<String>,
}

async fn create_customer(
    State(pool): State<Arc<DbPool>>,
    _user: AuthenticatedUser,
    Json(customer_info): Json<Customer>,
) -> Result<impl IntoResponse, ServiceError> {
    customer_info.validate()?;
    let created_customer = create_customer_service(&pool, customer_info).await?;
    Ok((axum::http::StatusCode::CREATED, Json(created_customer)))
}

async fn get_customer(
    State(pool): State<Arc<DbPool>>,
    Path(id): Path<i32>,
    _user: AuthenticatedUser,
) -> Result<impl IntoResponse, ServiceError> {
    let customer = get_customer_service(&pool, id).await?;
    Ok(Json(customer))
}

async fn update_customer(
    State(pool): State<Arc<DbPool>>,
    Path(id): Path<i32>,
    _user: AuthenticatedUser,
    Json(customer_info): Json<Customer>,
) -> Result<impl IntoResponse, ServiceError> {
    customer_info.validate()?;
    let updated_customer = update_customer_service(&pool, id, customer_info).await?;
    Ok(Json(updated_customer))
}

async fn delete_customer(
    State(pool): State<Arc<DbPool>>,
    Path(id): Path<i32>,
    _user: AuthenticatedUser,
) -> Result<impl IntoResponse, ServiceError> {
    delete_customer_service(&pool, id).await?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}

async fn list_customers(
    State(pool): State<Arc<DbPool>>,
    Query(query): Query<PaginationParams>,
    _user: AuthenticatedUser,
) -> Result<impl IntoResponse, ServiceError> {
    let customers = list_customers_service(&pool, query).await?;
    Ok(Json(customers))
}

async fn search_customers(
    State(pool): State<Arc<DbPool>>,
    Query(query): Query<CustomerSearchParams>,
    _user: AuthenticatedUser,
) -> Result<impl IntoResponse, ServiceError> {
    let customers = search_customers_service(&pool, query).await?;
    Ok(Json(customers))
}

async fn get_customer_orders(
    State(pool): State<Arc<DbPool>>,
    Path(id): Path<i32>,
    Query(query): Query<PaginationParams>,
    _user: AuthenticatedUser,
) -> Result<impl IntoResponse, ServiceError> {
    let orders = get_customer_orders_service(&pool, id, query).await?;
    Ok(Json(orders))
}

async fn get_customer_returns(
    State(pool): State<Arc<DbPool>>,
    Path(id): Path<i32>,
    Query(query): Query<PaginationParams>,
    _user: AuthenticatedUser,
) -> Result<impl IntoResponse, ServiceError> {
    let returns = get_customer_returns_service(&pool, id, query).await?;
    Ok(Json(returns))
}

pub fn customer_routes() -> Router {
    Router::new()
        .route("/", post(create_customer))
        .route("/", get(list_customers))
        .route("/search", get(search_customers))
        .route("/:id", get(get_customer))
        .route("/:id", put(update_customer))
        .route("/:id", delete(delete_customer))
}
