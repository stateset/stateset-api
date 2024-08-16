use axum::{
    extract::State,
    routing::get,
    Json,
    Router,
};
use crate::db::DbPool;
use crate::errors::ServiceError;
use crate::services::reports::{
    generate_sales_report,
    generate_inventory_report,
    generate_sales_by_product_report,
    generate_cogs_report,
    generate_work_order_efficiency_report,
};
use crate::auth::AuthenticatedUser;

async fn sales_report(
    State(pool): State<DbPool>,
    _user: AuthenticatedUser,
) -> Result<Json<impl serde::Serialize>, ServiceError> {
    let report = generate_sales_report(&pool).await?;
    Ok(Json(report))
}

async fn inventory_report(
    State(pool): State<DbPool>,
    _user: AuthenticatedUser,
) -> Result<Json<impl serde::Serialize>, ServiceError> {
    let report = generate_inventory_report(&pool).await?;
    Ok(Json(report))
}

async fn sales_by_product_report(
    State(pool): State<DbPool>,
    _user: AuthenticatedUser,
) -> Result<Json<impl serde::Serialize>, ServiceError> {
    let report = generate_sales_by_product_report(&pool).await?;
    Ok(Json(report))
}

async fn cogs_report(
    State(pool): State<DbPool>,
    _user: AuthenticatedUser,
) -> Result<Json<impl serde::Serialize>, ServiceError> {
    let report = generate_cogs_report(&pool).await?;
    Ok(Json(report))
}

async fn work_order_efficiency_report(
    State(pool): State<DbPool>,
    _user: AuthenticatedUser,
) -> Result<Json<impl serde::Serialize>, ServiceError> {
    let report = generate_work_order_efficiency_report(&pool).await?;
    Ok(Json(report))
}

pub fn reports_router() -> Router<DbPool> {
    Router::new()
        .route("/sales", get(sales_report))
        .route("/inventory", get(inventory_report))
        .route("/sales-by-product", get(sales_by_product_report))
        .route("/cogs", get(cogs_report))
        .route("/work-order-efficiency", get(work_order_efficiency_report))
}