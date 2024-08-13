use actix_web::{post, get, put, delete, web, HttpResponse};
use crate::db::DbPool;
use crate::models::customer::Customer;
use crate::errors::ServiceError;
use crate::services::customers::{create_customer, get_customer, update_customer, delete_customer, list_customers, search_customers, get_customer_orders, get_customer_returns};
use crate::auth::AuthenticatedUser;
use validator::Validate;

#[post("")]
async fn create_customer(
    pool: web::Data<DbPool>,
    customer_info: web::Json<Customer>,
    _user: AuthenticatedUser,
) -> Result<HttpResponse, ServiceError> {
    customer_info.validate()?;
    let created_customer = create_customer(&pool, customer_info.into_inner()).await?;
    Ok(HttpResponse::Created().json(created_customer))
}

#[get("/{id}")]
async fn get_customer(
    pool: web::Data<DbPool>,
    id: web::Path<i32>,
    _user: AuthenticatedUser,
) -> Result<HttpResponse, ServiceError> {
    let customer = get_customer(&pool, id.into_inner()).await?;
    Ok(HttpResponse::Ok().json(customer))
}

#[put("/{id}")]
async fn update_customer(
    pool: web::Data<DbPool>,
    id: web::Path<i32>,
    customer_info: web::Json<Customer>,
    _user: AuthenticatedUser,
) -> Result<HttpResponse, ServiceError> {
    customer_info.validate()?;
    let updated_customer = update_customer(&pool, id.into_inner(), customer_info.into_inner()).await?;
    Ok(HttpResponse::Ok().json(updated_customer))
}

#[delete("/{id}")]
async fn delete_customer(
    pool: web::Data<DbPool>,
    id: web::Path<i32>,
    _user: AuthenticatedUser,
) -> Result<HttpResponse, ServiceError> {
    delete_customer(&pool, id.into_inner()).await?;
    Ok(HttpResponse::NoContent().finish())
}

#[get("")]
async fn list_customers(
    pool: web::Data<DbPool>,
    query: web::Query<PaginationParams>,
    _user: AuthenticatedUser,
) -> Result<HttpResponse, ServiceError> {
    let customers = list_customers(&pool, query.into_inner()).await?;
    Ok(HttpResponse::Ok().json(customers))
}

#[get("/search")]
async fn search_customers(
    pool: web::Data<DbPool>,
    query: web::Query<CustomerSearchParams>,
    _user: AuthenticatedUser,
) -> Result<HttpResponse, ServiceError> {
    let customers = search_customers(&pool, query.into_inner()).await?;
    Ok(HttpResponse::Ok().json(customers))
}

#[get("/{id}/orders")]
async fn get_customer_orders(
    pool: web::Data<DbPool>,
    id: web::Path<i32>,
    query: web::Query<PaginationParams>,
    _user: AuthenticatedUser,
) -> Result<HttpResponse, ServiceError> {
    let orders = get_customer_orders(&pool, id.into_inner(), query.into_inner()).await?;
    Ok(HttpResponse::Ok().json(orders))
}

#[get("/{id}/returns")]
async fn get_customer_returns(
    pool: web::Data<DbPool>,
    id: web::Path<i32>,
    query: web::Query<PaginationParams>,
    _user: AuthenticatedUser,
) -> Result<HttpResponse, ServiceError> {
    let returns = get_customer_returns(&pool, id.into_inner(), query.into_inner()).await?;
    Ok(HttpResponse::Ok().json(returns))
}