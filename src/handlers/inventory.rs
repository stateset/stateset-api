use actix_web::{post, get, put, delete, web, HttpResponse};
use crate::db::DbPool;
use crate::models::inventory::{NewProduct, Product, ProductSearchParams, StockAdjustment};
use crate::errors::ServiceError;
use crate::services::inventory::{create_product, get_product, update_product, delete_product, list_products, search_products, adjust_stock};
use crate::auth::AuthenticatedUser;
use validator::Validate;

#[post("")]
async fn create_product(
    pool: web::Data<DbPool>,
    product_info: web::Json<NewProduct>,
    _user: AuthenticatedUser,
) -> Result<HttpResponse, ServiceError> {
    product_info.validate()?;
    let created_product = create_product(&pool, product_info.into_inner()).await?;
    Ok(HttpResponse::Created().json(created_product))
}

#[get("/{id}")]
async fn get_product(
    pool: web::Data<DbPool>,
    id: web::Path<i32>,
    _user: AuthenticatedUser,
) -> Result<HttpResponse, ServiceError> {
    let product = get_product(&pool, id.into_inner()).await?;
    Ok(HttpResponse::Ok().json(product))
}

#[put("/{id}")]
async fn update_product(
    pool: web::Data<DbPool>,
    id: web::Path<i32>,
    product_info: web::Json<Product>,
    _user: AuthenticatedUser,
) -> Result<HttpResponse, ServiceError> {
    product_info.validate()?;
    let updated_product = update_product(&pool, id.into_inner(), product_info.into_inner()).await?;
    Ok(HttpResponse::Ok().json(updated_product))
}

#[delete("/{id}")]
async fn delete_product(
    pool: web::Data<DbPool>,
    id: web::Path<i32>,
    _user: AuthenticatedUser,
) -> Result<HttpResponse, ServiceError> {
    delete_product(&pool, id.into_inner()).await?;
    Ok(HttpResponse::NoContent().finish())
}

#[get("")]
async fn list_products(
    pool: web::Data<DbPool>,
    query: web::Query<PaginationParams>,
    _user: AuthenticatedUser,
) -> Result<HttpResponse, ServiceError> {
    let products = list_products(&pool, query.into_inner()).await?;
    Ok(HttpResponse::Ok().json(products))
}

#[get("/search")]
async fn search_products(
    pool: web::Data<DbPool>,
    query: web::Query<ProductSearchParams>,
    _user: AuthenticatedUser,
) -> Result<HttpResponse, ServiceError> {
    let products = search_products(&pool, query.into_inner()).await?;
    Ok(HttpResponse::Ok().json(products))
}

#[post("/adjust-stock")]
async fn adjust_stock(
    pool: web::Data<DbPool>,
    adjustment: web::Json<StockAdjustment>,
    _user: AuthenticatedUser,
) -> Result<HttpResponse, ServiceError> {
    adjustment.validate()?;
    let updated_product = adjust_stock(&pool, adjustment.into_inner()).await?;
    Ok(HttpResponse::Ok().json(updated_product))
}

#[get("/low-stock")]
async fn get_low_stock_products(
    pool: web::Data<DbPool>,
    _user: AuthenticatedUser,
) -> Result<HttpResponse, ServiceError> {
    let low_stock_products = get_low_stock_products(&pool).await?;
    Ok(HttpResponse::Ok().json(low_stock_products))
}