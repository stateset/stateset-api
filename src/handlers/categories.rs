use actix_web::{post, get, put, delete, web, HttpResponse};
use crate::db::DbPool;
use crate::models::product_category::{ProductCategory, ProductCategoryAssociation};
use crate::errors::ServiceError;
use crate::services::categories::{create_category, get_category, update_category, delete_category, list_categories, get_category_products};
use crate::auth::AuthenticatedUser;
use validator::Validate;

#[post("")]
async fn create_category(
    pool: web::Data<DbPool>,
    category_info: web::Json<ProductCategory>,
    _user: AuthenticatedUser,
) -> Result<HttpResponse, ServiceError> {
    category_info.validate()?;
    let created_category = create_category(&pool, category_info.into_inner()).await?;
    Ok(HttpResponse::Created().json(created_category))
}

#[get("/{id}")]
async fn get_category(
    pool: web::Data<DbPool>,
    id: web::Path<i32>,
    _user: AuthenticatedUser,
) -> Result<HttpResponse, ServiceError> {
    let category = get_category(&pool, id.into_inner()).await?;
    Ok(HttpResponse::Ok().json(category))
}

#[put("/{id}")]
async fn update_category(
    pool: web::Data<DbPool>,
    id: web::Path<i32>,
    category_info: web::Json<ProductCategory>,
    _user: AuthenticatedUser,
) -> Result<HttpResponse, ServiceError> {
    category_info.validate()?;
    let updated_category = update_category(&pool, id.into_inner(), category_info.into_inner()).await?;
    Ok(HttpResponse::Ok().json(updated_category))
}

#[delete("/{id}")]
async fn delete_category(
    pool: web::Data<DbPool>,
    id: web::Path<i32>,
    _user: AuthenticatedUser,
) -> Result<HttpResponse, ServiceError> {
    delete_category(&pool, id.into_inner()).await?;
    Ok(HttpResponse::NoContent().finish())
}

#[get("")]
async fn list_categories(
    pool: web::Data<DbPool>,
    _user: AuthenticatedUser,
) -> Result<HttpResponse, ServiceError> {
    let categories = list_categories(&pool).await?;
    Ok(HttpResponse::Ok().json(categories))
}

#[get("/{id}/products")]
async fn get_category_products(
    pool: web::Data<DbPool>,
    id: web::Path<i32>,
    query: web::Query<PaginationParams>,
    _user: AuthenticatedUser,
) -> Result<HttpResponse, ServiceError> {
    let products = get_category_products(&pool, id.into_inner(), query.into_inner()).await?;
    Ok(HttpResponse::Ok().json(products))
}
