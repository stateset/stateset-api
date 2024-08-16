use axum::{
    routing::{post, get, put, delete},
    extract::{State, Path, Query, Json},
    response::IntoResponse,
    Router,
};
use crate::db::DbPool;
use crate::models::customer::Customer;
use crate::errors::ServiceError;
use crate::services::customers::{create_customer, get_customer, update_customer, delete_customer, list_customers, search_customers, get_customer_orders, get_customer_returns};
use crate::auth::AuthenticatedUser;
use crate::utils::pagination::PaginationParams;
use validator::Validate;
use std::sync::Arc;

async fn create_customer(
    State(pool): State<Arc<DbPool>>,
    AuthenticatedUser(_user): AuthenticatedUser,
    Json(customer_info): Json<Customer>,
) -> Result<impl IntoResponse, ServiceError> {
    customer_info.validate()?;
    let created_customer = create_customer(&pool, customer_info).await?;
    Ok((axum::http::StatusCode::CREATED, Json(created_customer)))
}

async fn get_customer(
    State(pool): State<Arc<DbPool>>,
    Path(id): Path<i32>,
    AuthenticatedUser(_user): AuthenticatedUser,
) -> Result<impl IntoResponse, ServiceError> {
    let customer = get_customer(&pool, id).await?;
    Ok(Json(customer))
}

async fn update_customer(
    State(pool): State<Arc<DbPool>>,
    Path(id): Path<i32>,
    AuthenticatedUser(_user): AuthenticatedUser,
    Json(customer_info): Json<Customer>,
) -> Result<impl IntoResponse, ServiceError> {
    customer_info.validate()?;
    let updated_customer = update_customer(&pool, id, customer_info).await?;
    Ok(Json(updated_customer))
}

async fn delete_customer(
    State(pool): State<Arc<DbPool>>,
    Path(id): Path<i32>,
    AuthenticatedUser(_user): AuthenticatedUser,
) -> Result<impl IntoResponse, ServiceError> {
    delete_customer(&pool, id).await?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}

async fn list_customers(
    State(pool): State<Arc<DbPool>>,
    Query(query): Query<PaginationParams>,
    AuthenticatedUser(_user): AuthenticatedUser,
) -> Result<impl IntoResponse, ServiceError> {
    let customers = list_customers(&pool, query).await?;
    Ok(Json(customers))
}

async fn search_customers(
    State(pool): State<Arc<DbPool>>,
    Query(query): Query<CustomerSearchParams>,
    AuthenticatedUser(_user): AuthenticatedUser,
) -> Result<impl IntoResponse, ServiceError> {
    let customers = search_customers(&pool, query).await?;
    Ok(Json(customers))
}

async fn get_customer_orders(
    State(pool): State<Arc<DbPool>>,
    Path(id): Path<i32>,
    Query(query): Query<PaginationParams>,
    AuthenticatedUser(_user): AuthenticatedUser,
) -> Result<impl IntoResponse, ServiceError> {
    let orders = get_customer_orders(&pool, id, query).await?;
    Ok(Json(orders))
}

async fn get_customer_returns(
    State(pool): State<Arc<DbPool>>,
    Path(id): Path<i32>,
    Query(query): Query<PaginationParams>,
    AuthenticatedUser(_user): AuthenticatedUser,
) -> Result<impl IntoResponse, ServiceError> {
    let returns = get_customer_returns(&pool, id, query).await?;
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