use axum::{
    extract::{Path, Query, State},
    routing::{get, post, put, delete},
    Json, Router,
};
use crate::db::DbPool;
use crate::models::shipment::{NewShipment, Shipment, ShipmentSearchParams};
use crate::errors::ServiceError;
use crate::services::shipments::{create_shipment, get_shipment, update_shipment, delete_shipment, list_shipments, search_shipments};
use crate::auth::AuthenticatedUser;
use validator::Validate;

async fn create_shipment_handler(
    State(pool): State<DbPool>,
    Json(shipment_info): Json<NewShipment>,
    _user: AuthenticatedUser,
) -> Result<Json<Shipment>, ServiceError> {
    shipment_info.validate()?;
    let created_shipment = create_shipment(&pool, shipment_info).await?;
    Ok(Json(created_shipment))
}

async fn get_shipment_handler(
    State(pool): State<DbPool>,
    Path(id): Path<i32>,
    _user: AuthenticatedUser,
) -> Result<Json<Shipment>, ServiceError> {
    let shipment = get_shipment(&pool, id).await?;
    Ok(Json(shipment))
}

async fn update_shipment_handler(
    State(pool): State<DbPool>,
    Path(id): Path<i32>,
    Json(shipment_info): Json<Shipment>,
    _user: AuthenticatedUser,
) -> Result<Json<Shipment>, ServiceError> {
    shipment_info.validate()?;
    let updated_shipment = update_shipment(&pool, id, shipment_info).await?;
    Ok(Json(updated_shipment))
}

async fn delete_shipment_handler(
    State(pool): State<DbPool>,
    Path(id): Path<i32>,
    _user: AuthenticatedUser,
) -> Result<(), ServiceError> {
    delete_shipment(&pool, id).await?;
    Ok(())
}

async fn list_shipments_handler(
    State(pool): State<DbPool>,
    Query(params): Query<PaginationParams>,
    _user: AuthenticatedUser,
) -> Result<Json<Vec<Shipment>>, ServiceError> {
    let shipments = list_shipments(&pool, params).await?;
    Ok(Json(shipments))
}

async fn search_shipments_handler(
    State(pool): State<DbPool>,
    Query(params): Query<ShipmentSearchParams>,
    _user: AuthenticatedUser,
) -> Result<Json<Vec<Shipment>>, ServiceError> {
    let shipments = search_shipments(&pool, params).await?;
    Ok(Json(shipments))
}

pub fn shipment_routes() -> Router<DbPool> {
    Router::new()
        .route("/", post(create_shipment_handler))
        .route("/:id", get(get_shipment_handler))
        .route("/:id", put(update_shipment_handler))
        .route("/:id", delete(delete_shipment_handler))
        .route("/", get(list_shipments_handler))
        .route("/search", get(search_shipments_handler))
        .route("/:id/assign", post(assign_shipment_handler))
        .route("/:id/cancel", post(cancel_shipment_handler))

}