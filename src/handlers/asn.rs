use axum::{
    routing::{post, get, put, delete},
    extract::{State, Path, Query, Json},
    response::IntoResponse,
    Router,
};
use crate::db::DbPool;
use crate::models::asn::{NewASN, ASN};
use crate::errors::ServiceError;
use crate::auth::AuthenticatedUser;
use crate::utils::pagination::PaginationParams;
use validator::Validate;
use std::sync::Arc;

use crate::commands::asn::{
    CreateASNCommand,
    UpdateASNCommand,
    DeleteASNCommand,
    InTransitASNCommand,
    DeliveredASNCommand,
    CancelASNCommand,
};

async fn create_asn(
    State(pool): State<Arc<DbPool>>,
    AuthenticatedUser(_user): AuthenticatedUser,
    Json(asn_info): Json<NewASN>,
) -> Result<impl IntoResponse, ServiceError> {
    asn_info.validate()?;
    let created_asn = CreateASNCommand {
        asn_info,
        user_id: user.user_id,
    }.execute(&pool).await?;
    Ok((axum::http::StatusCode::CREATED, Json(created_asn)))
}

async fn list_asns(
    State(pool): State<Arc<DbPool>>,
    AuthenticatedUser(_user): AuthenticatedUser,
) -> Result<impl IntoResponse, ServiceError> {
    let asns = list_asns(&pool).await?;
    Ok(Json(asns))
}

async fn get_asn(
    State(pool): State<Arc<DbPool>>,
    Path(id): Path<i32>,
    AuthenticatedUser(_user): AuthenticatedUser,
) -> Result<impl IntoResponse, ServiceError> {
    let asn = get_asn(&pool, id).await?;
    Ok(Json(asn))
}

async fn update_asn(
    State(pool): State<Arc<DbPool>>,
    Path(id): Path<i32>,
    AuthenticatedUser(_user): AuthenticatedUser,
    Json(asn_info): Json<ASN>,
) -> Result<impl IntoResponse, ServiceError> {
    let updated_asn = update_asn(&pool, id, asn_info).await?;
    Ok(Json(updated_asn))
}

async fn delete_asn(
    State(pool): State<Arc<DbPool>>,
    Path(id): Path<i32>,
    AuthenticatedUser(_user): AuthenticatedUser,
) -> Result<impl IntoResponse, ServiceError> {
    delete_asn(&pool, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn in_transit_asn(
    State(pool): State<Arc<DbPool>>,
    Path(id): Path<i32>,
    AuthenticatedUser(_user): AuthenticatedUser,
) -> Result<impl IntoResponse, ServiceError> {
    let updated_asn = InTransitASNCommand {
        asn_id: id,
        user_id: user.user_id,
    }.execute(&pool).await?;
    Ok(Json(updated_asn))
}

async fn delivered_asn(
    State(pool): State<Arc<DbPool>>,
    Path(id): Path<i32>,
    AuthenticatedUser(_user): AuthenticatedUser,
) -> Result<impl IntoResponse, ServiceError> {
    let updated_asn = DeliveredASNCommand {
        asn_id: id,
        user_id: user.user_id,
    }.execute(&pool).await?;
    Ok(Json(updated_asn))
}

async fn cancel_asn(
    State(pool): State<Arc<DbPool>>,
    Path(id): Path<i32>,
    AuthenticatedUser(_user): AuthenticatedUser,
) -> Result<impl IntoResponse, ServiceError> {
    let updated_asn = CancelASNCommand {
        asn_id: id,
        user_id: user.user_id,
    }.execute(&pool).await?;
    Ok(Json(updated_asn))
}

pub fn asn_routes() -> Router {
    Router::new()
        .route("/", post(create_asn))
        .route("/", get(list_asns))
        .route("/:id", get(get_asn))
        .route("/:id", put(update_asn))
        .route("/:id", delete(delete_asn))
        .route("/:id/in-transit", post(in_transit_asn))
        .route("/:id/delivered", post(delivered_asn))
        .route("/:id/cancel", post(cancel_asn))
}
