use axum::{
    routing::{post, get, put, delete},
    extract::{State, Path, Query, Json},
    response::IntoResponse,
    Router,
};
use crate::db::DbPool;
use crate::models::inventory::{NewProduct, Product, ProductSearchParams, StockAdjustment};
use crate::errors::ServiceError;
use crate::services::inventory::{create_product, get_product, update_product, delete_product, list_products, search_products, adjust_stock, get_low_stock_products};
use crate::auth::AuthenticatedUser;
use crate::utils::pagination::PaginationParams;
use validator::Validate;
use std::sync::Arc;

use crate::commands::inventory::{
    CreateProductCommand,
    UpdateProductCommand,
    DeleteProductCommand,
    ReserveInventoryCommand,
    ReleaseInventoryCommand,
};

async fn create_product(
    State(pool): State<Arc<DbPool>>,
    AuthenticatedUser(_user): AuthenticatedUser,
    Json(product_info): Json<NewProduct>,
) -> Result<impl IntoResponse, ServiceError> {
    product_info.validate()?;
    let created_product = CreateProductCommand {
        product_info,
        user_id: user.user_id,
    }.execute(&pool).await?;
    Ok((axum::http::StatusCode::CREATED, Json(created_product)))
}

async fn get_product(
    State(pool): State<Arc<DbPool>>,
    Path(id): Path<i32>,
    AuthenticatedUser(_user): AuthenticatedUser,
) -> Result<impl IntoResponse, ServiceError> {
    let product = get_product(&pool, id).await?;
    Ok(Json(product))
}

async fn update_product(
    State(pool): State<Arc<DbPool>>,
    Path(id): Path<i32>,
    AuthenticatedUser(_user): AuthenticatedUser,
    Json(product_info): Json<Product>,
) -> Result<impl IntoResponse, ServiceError> {
    product_info.validate()?;
    let updated_product = update_product(&pool, id, product_info).await?;
    Ok(Json(updated_product))
}

async fn delete_product(
    State(pool): State<Arc<DbPool>>,
    Path(id): Path<i32>,
    AuthenticatedUser(_user): AuthenticatedUser,
) -> Result<impl IntoResponse, ServiceError> {
    delete_product(&pool, id).await?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}

async fn list_products(
    State(pool): State<Arc<DbPool>>,
    Query(query): Query<PaginationParams>,
    AuthenticatedUser(_user): AuthenticatedUser,
) -> Result<impl IntoResponse, ServiceError> {
    let products = list_products(&pool, query).await?;
    Ok(Json(products))
}

async fn search_products(
    State(pool): State<Arc<DbPool>>,
    Query(query): Query<ProductSearchParams>,
    AuthenticatedUser(_user): AuthenticatedUser,
) -> Result<impl IntoResponse, ServiceError> {
    let products = search_products(&pool, query).await?;
    Ok(Json(products))
}

async fn adjust_stock(
    State(pool): State<Arc<DbPool>>,
    AuthenticatedUser(_user): AuthenticatedUser,
    Json(adjustment): Json<StockAdjustment>,
) -> Result<impl IntoResponse, ServiceError> {
    adjustment.validate()?;
    let updated_product = adjust_stock(&pool, adjustment).await?;
    Ok(Json(updated_product))
}

async fn get_low_stock_products(
    State(pool): State<Arc<DbPool>>,
    AuthenticatedUser(_user): AuthenticatedUser,
) -> Result<impl IntoResponse, ServiceError> {
    let low_stock_products = get_low_stock_products(&pool).await?;
    Ok(Json(low_stock_products))
}

async fn get_inventory_movement(
    State(pool): State<Arc<DbPool>>,
    AuthenticatedUser(_user): AuthenticatedUser,
) -> Result<impl IntoResponse, ServiceError> {
    let inventory_movement = get_inventory_movement(&pool).await?;
    Ok(Json(inventory_movement))
}

async fn reserve_inventory(
    State(pool): State<Arc<DbPool>>,
    AuthenticatedUser(_user): AuthenticatedUser,
    Json(command): Json<ReserveInventoryCommand>,
) -> Result<impl IntoResponse, ServiceError> {
    let result = ReserveInventoryCommand {
        command,
        user_id: user.user_id,
    }.execute(&pool).await?;
    Ok(Json(result))
}

async fn release_inventory(
    State(pool): State<Arc<DbPool>>,
    AuthenticatedUser(_user): AuthenticatedUser,
    Json(command): Json<ReleaseInventoryCommand>,
) -> Result<impl IntoResponse, ServiceError> {
    let result = ReleaseInventoryCommand {
        command,
        user_id: user.user_id,
    }.execute(&pool).await?;
    Ok(Json(result))
}

pub fn inventory_routes() -> Router {
    Router::new()
        .route("/", post(create_product))
        .route("/", get(list_products))
        .route("/search", get(search_products))
        .route("/:id", get(get_product))
        .route("/:id", put(update_product))
        .route("/:id", delete(delete_product))
        .route("/adjust", post(adjust_stock))
        .route("/low-stock", get(get_low_stock_products))
        .route("/reserve", post(reserve_inventory))
        .route("/release", post(release_inventory))
        .route("/movement", get(get_inventory_movement))
}
