use actix_web::{post, get, put, delete, web, HttpResponse};
use crate::db::DbPool;
use crate::models::work_order::{NewWorkOrder, WorkOrder, WorkOrderSearchParams, WorkOrderStatus};
use crate::errors::ServiceError;
use crate::services::work_orders::{create_work_order, get_work_order, update_work_order, delete_work_order, list_work_orders, search_work_orders, assign_work_order, complete_work_order};
use crate::auth::AuthenticatedUser;
use validator::Validate;

#[post("")]
async fn create_work_order(
    pool: web::Data<DbPool>,
    work_order_info: web::Json<NewWorkOrder>,
    user: AuthenticatedUser,
) -> Result<HttpResponse, ServiceError> {
    work_order_info.validate()?;
    let created_work_order = create_work_order(&pool, work_order_info.into_inner(), user.user_id).await?;
    Ok(HttpResponse::Created().json(created_work_order))
}

#[get("/{id}")]
async fn get_work_order(
    pool: web::Data<DbPool>,
    id: web::Path<i32>,
    _user: AuthenticatedUser,
) -> Result<HttpResponse, ServiceError> {
    let work_order = get_work_order(&pool, id.into_inner()).await?;
    Ok(HttpResponse::Ok().json(work_order))
}

#[put("/{id}")]
async fn update_work_order(
    pool: web::Data<DbPool>,
    id: web::Path<i32>,
    work_order_info: web::Json<WorkOrder>,
    _user: AuthenticatedUser,
) -> Result<HttpResponse, ServiceError> {
    work_order_info.validate()?;
    let updated_work_order = update_work_order(&pool, id.into_inner(), work_order_info.into_inner()).await?;
    Ok(HttpResponse::Ok().json(updated_work_order))
}

#[delete("/{id}")]
async fn delete_work_order(
    pool: web::Data<DbPool>,
    id: web::Path<i32>,
    _user: AuthenticatedUser,
) -> Result<HttpResponse, ServiceError> {
    delete_work_order(&pool, id.into_inner()).await?;
    Ok(HttpResponse::NoContent().finish())
}

#[get("")]
async fn list_work_orders(
    pool: web::Data<DbPool>,
    query: web::Query<PaginationParams>,
    _user: AuthenticatedUser,
) -> Result<HttpResponse, ServiceError> {
    let work_orders = list_work_orders(&pool, query.into_inner()).await?;
    Ok(HttpResponse::Ok().json(work_orders))
}

#[get("/search")]
async fn search_work_orders(
    pool: web::Data<DbPool>,
    query: web::Query<WorkOrderSearchParams>,
    _user: AuthenticatedUser,
) -> Result<HttpResponse, ServiceError> {
    let work_orders = search_work_orders(&pool, query.into_inner()).await?;
    Ok(HttpResponse::Ok().json(work_orders))
}

#[post("/{id}/assign")]
async fn assign_work_order(
    pool: web::Data<DbPool>,
    id: web::Path<i32>,
    user_id: web::Json<i32>,
    _user: AuthenticatedUser,
) -> Result<HttpResponse, ServiceError> {
    let updated_work_order = assign_work_order(&pool, id.into_inner(), user_id.into_inner()).await?;
    Ok(HttpResponse::Ok().json(updated_work_order))
}

#[post("/{id}/complete")]
async fn complete_work_order(
    pool: web::Data<DbPool>,
    id: web::Path<i32>,
    actual_duration: web::Json<i32>,
    user: AuthenticatedUser,
) -> Result<HttpResponse, ServiceError> {
    let completed_work_order = complete_work_order(&pool, id.into_inner(), actual_duration.into_inner(), user.user_id).await?;
    Ok(HttpResponse::Ok().json(completed_work_order))
}