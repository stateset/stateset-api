use super::common::PaginationParams;
use crate::auth::AuthenticatedUser;
use crate::db::DbPool;
use crate::errors::ServiceError;
use crate::models::{asn_entity::{self, Entity as ASNEntity, Model as ASN}};
use crate::handlers::AppState;
use axum::{
    extract::{Json, Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, post, put},
    Router,
};
use std::sync::Arc;
use validator::Validate;
use sea_orm::{EntityTrait, QueryFilter, Set, ActiveModelTrait};
use uuid::Uuid;

use crate::commands::advancedshippingnotice::{
    CancelASNCommand, CreateASNCommand, DeliveredASNCommand, InTransitASNCommand,
    MarkASNDeliveredCommand, MarkASNInTransitCommand, UpdateAsnDetailsCommand,
};
use crate::commands::Command;

async fn create_asn(
    State(pool): State<Arc<DbPool>>,
    user: AuthenticatedUser,
    Json(command): Json<CreateASNCommand>,
) -> Result<impl IntoResponse, ServiceError> {
    command.validate()?;
    let event_sender = Arc::new(crate::events::EventSender::new());
    let created_asn = command.execute(Arc::new(pool.as_ref().clone()), event_sender).await?;
    Ok((StatusCode::CREATED, Json(created_asn)))
}

async fn list_asns(
    State(pool): State<Arc<DbPool>>,
    _user: AuthenticatedUser,
) -> Result<impl IntoResponse, ServiceError> {
    let asns = fetch_all_asns(&pool).await?;
    Ok(Json(asns))
}

async fn get_asn(
    State(pool): State<Arc<DbPool>>,
    Path(id): Path<Uuid>,
    _user: AuthenticatedUser,
) -> Result<impl IntoResponse, ServiceError> {
    let asn = fetch_asn_by_id(&pool, id).await?;
    Ok(Json(asn))
}

// Database functions
async fn fetch_all_asns(pool: &DbPool) -> Result<Vec<ASN>, ServiceError> {
    ASNEntity::find()
        .all(pool)
        .await
        .map_err(|e| {
            tracing::error!("Error fetching all ASNs: {:?}", e);
            ServiceError::DatabaseError(e)
        })
}

async fn fetch_asn_by_id(pool: &DbPool, id: Uuid) -> Result<ASN, ServiceError> {
    ASNEntity::find_by_id(id)
        .one(pool)
        .await
        .map_err(|e| {
            tracing::error!("Error fetching ASN by ID: {:?}", e);
            ServiceError::DatabaseError(e)
        })?
        .ok_or_else(|| {
            tracing::error!("ASN with ID {} not found", id);
            ServiceError::NotFound
        })
}

async fn update_asn_in_db(pool: &DbPool, id: Uuid, asn_info: ASN) -> Result<ASN, ServiceError> {
    // First check if the ASN exists
    let existing_asn = fetch_asn_by_id(pool, id).await?;
    
    // Create an active model from the existing ASN
    let mut asn_active_model: asn_entity::ActiveModel = existing_asn.into();
    
    // Update the fields from asn_info
    asn_active_model.asn_number = Set(asn_info.asn_number);
    asn_active_model.status = Set(asn_info.status);
    asn_active_model.supplier_id = Set(asn_info.supplier_id);
    asn_active_model.supplier_name = Set(asn_info.supplier_name);
    asn_active_model.expected_delivery_date = Set(asn_info.expected_delivery_date);
    asn_active_model.shipping_date = Set(asn_info.shipping_date);
    asn_active_model.carrier_type = Set(asn_info.carrier_type);
    asn_active_model.tracking_number = Set(asn_info.tracking_number);
    asn_active_model.shipping_address = Set(asn_info.shipping_address);
    asn_active_model.notes = Set(asn_info.notes);
    asn_active_model.updated_at = Set(chrono::Utc::now());
    asn_active_model.version = Set(existing_asn.version + 1);
    
    // Save the updated ASN
    asn_active_model
        .update(pool)
        .await
        .map_err(|e| {
            tracing::error!("Error updating ASN: {:?}", e);
            ServiceError::DatabaseError(e)
        })
}

async fn delete_asn_from_db(pool: &DbPool, id: Uuid) -> Result<(), ServiceError> {
    // First check if the ASN exists
    let _existing_asn = fetch_asn_by_id(pool, id).await?;
    
    // Delete the ASN
    ASNEntity::delete_by_id(id)
        .exec(pool)
        .await
        .map_err(|e| {
            tracing::error!("Error deleting ASN: {:?}", e);
            ServiceError::DatabaseError(e)
        })?;
    
    Ok(())
}

async fn update_asn(
    State(pool): State<Arc<DbPool>>,
    Path(id): Path<Uuid>,
    _user: AuthenticatedUser,
    Json(asn_info): Json<ASN>,
) -> Result<impl IntoResponse, ServiceError> {
    let updated_asn = update_asn_in_db(&pool, id, asn_info).await?;
    Ok(Json(updated_asn))
}

async fn delete_asn(
    State(pool): State<Arc<DbPool>>,
    Path(id): Path<Uuid>,
    _user: AuthenticatedUser,
) -> Result<impl IntoResponse, ServiceError> {
    delete_asn_from_db(&pool, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn in_transit_asn(
    State(pool): State<Arc<DbPool>>,
    Path(id): Path<Uuid>,
    user: AuthenticatedUser,
) -> Result<impl IntoResponse, ServiceError> {
    let event_sender = Arc::new(crate::events::EventSender::new());
    let updated_asn = InTransitASNCommand {
        asn_id: id,
        user_id: user.user_id,
    }
    .execute(Arc::new(pool.as_ref().clone()), event_sender)
    .await?;
    Ok(Json(updated_asn))
}

async fn delivered_asn(
    State(pool): State<Arc<DbPool>>,
    Path(id): Path<Uuid>,
    user: AuthenticatedUser,
) -> Result<impl IntoResponse, ServiceError> {
    let event_sender = Arc::new(crate::events::EventSender::new());
    let updated_asn = DeliveredASNCommand {
        asn_id: id,
        user_id: user.user_id,
    }
    .execute(Arc::new(pool.as_ref().clone()), event_sender)
    .await?;
    Ok(Json(updated_asn))
}

async fn cancel_asn(
    State(pool): State<Arc<DbPool>>,
    Path(id): Path<Uuid>,
    user: AuthenticatedUser,
) -> Result<impl IntoResponse, ServiceError> {
    let event_sender = Arc::new(crate::events::EventSender::new());
    let updated_asn = CancelASNCommand {
        asn_id: id,
        user_id: user.user_id,
    }
    .execute(Arc::new(pool.as_ref().clone()), event_sender)
    .await?;
    Ok(Json(updated_asn))
}

pub fn asn_routes() -> Router<Arc<AppState>> {
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
