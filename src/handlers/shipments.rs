use actix_web::{post, get, put, delete, web, HttpResponse};
use crate::db::DbPool;
use crate::models::shipment::{NewShipment, Shipment, ShipmentSearchParams};
use crate::errors::ServiceError;
use crate::services::shipments::{create_shipment, get_shipment, update_shipment, delete_shipment, list_shipments, search_shipments};
use crate::auth::AuthenticatedUser;
use validator::Validate;

#[post("")]
async fn create_shipment(
    pool: web::Data<DbPool>,
    shipment_info: web::Json<NewShipment>,
    _user: AuthenticatedUser,
) -> Result<HttpResponse, ServiceError> {
    shipment_info.validate()?;
    let created_shipment = create_shipment(&pool, shipment_info.into_inner()).await?;
    Ok(HttpResponse::Created().json(created_shipment))
}

#[get("/{id}")]
async fn get_shipment(
    pool: web::Data<DbPool>,
    id: web::Path<i32>,
    _user: AuthenticatedUser,
) -> Result<HttpResponse, ServiceError> {
    let shipment = get_shipment(&pool, id.into_inner()).await?;
    Ok(HttpResponse::Ok().json(shipment))
}

#[put("/{id}")]
async fn update_shipment(
    pool: web::Data<DbPool>,
    id: web::Path<i32>,
    shipment_info: web::Json<Shipment>,
    _user: AuthenticatedUser,
) -> Result<HttpResponse, ServiceError> {
    shipment_info.validate()?;
    let updated_shipment = update_shipment(&pool, id.into_inner(), shipment_info.into_inner()).await?;
    Ok(HttpResponse::Ok().json(updated_shipment))
}

#[delete("/{id}")]
async fn delete_shipment(
    pool: web::Data<DbPool>,
    id: web::Path<i32>,
    _user: AuthenticatedUser,
) -> Result<HttpResponse, ServiceError> {
    delete_shipment(&pool, id.into_inner()).await?;
    Ok(HttpResponse::NoContent().finish())
}

#[get("")]
async fn list_shipments(
    pool: web::Data<DbPool>,
    query: web::Query<PaginationParams>,
    _user: AuthenticatedUser,
) -> Result<HttpResponse, ServiceError> {
    let shipments = list_shipments(&pool, query.into_inner()).await?;
    Ok(HttpResponse::Ok().json(shipments))
}

#[get("/search")]
async fn search_shipments(
    pool: web::Data<DbPool>,
    query: web::Query<ShipmentSearchParams>,
    _user: AuthenticatedUser,
) -> Result<HttpResponse, ServiceError> {
    let shipments = search_shipments(&pool, query.into_inner()).await?;
    Ok(HttpResponse::Ok().json(shipments))
}