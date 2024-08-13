use actix_web::{get, web, HttpResponse};
use crate::db::DbPool;
use crate::errors::ServiceError;
use crate::services::reports::{generate_sales_report, generate_inventory_report, generate_work_order_efficiency_report};
use crate::auth::AuthenticatedUser;

#[get("/sales")]
async fn generate_sales_report(
    pool: web::Data<DbPool>,
    _user: AuthenticatedUser,
) -> Result<HttpResponse, ServiceError> {
    let report = generate_sales_report(&pool).await?;
    Ok(HttpResponse::Ok().json(report))
}

#[get("/inventory")]
async fn generate_inventory_report(
    pool: web::Data<DbPool>,
    _user: AuthenticatedUser,
) -> Result<HttpResponse, ServiceError> {
    let report = generate_inventory_report(&pool).await?;
    Ok(HttpResponse::Ok().json(report))
}

#[get("/work-order-efficiency")]
async fn generate_work_order_efficiency_report(
    pool: web::Data<DbPool>,
    _user: AuthenticatedUser,
) -> Result<HttpResponse, ServiceError> {
    let report = generate_work_order_efficiency_report(&pool).await?;
    Ok(HttpResponse::Ok().json(report))
}