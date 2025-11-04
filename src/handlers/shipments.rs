use crate::{
    commands::shipments::create_shipment_command::CreateShipmentCommand,
    commands::shipments::track_shipment_command::TrackShipmentCommand, errors::ServiceError,
    models::shipment, ApiResponse, ApiResult, AppState, PaginatedResponse,
};
use axum::{
    extract::{Path, Query, State},
    response::Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use utoipa::ToSchema;
use uuid::Uuid;
use validator::Validate;

#[derive(Debug, Deserialize, Default, ToSchema)]
pub struct ShipmentListQuery {
    pub page: Option<u64>,
    pub limit: Option<u64>,
    pub status: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ShipmentSummary {
    pub id: Uuid,
    pub order_id: Uuid,
    pub tracking_number: String,
    pub status: String,
    pub shipping_method: String,
    pub shipping_address: String,
    pub recipient_name: String,
    pub estimated_delivery: Option<DateTime<Utc>>,
    pub shipped_at: Option<DateTime<Utc>>,
    pub delivered_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<shipment::Model> for ShipmentSummary {
    fn from(model: shipment::Model) -> Self {
        Self {
            id: model.id,
            order_id: model.order_id,
            tracking_number: model.tracking_number,
            status: model.status.to_string(),
            shipping_method: model.shipping_method.to_string(),
            shipping_address: model.shipping_address,
            recipient_name: model.recipient_name,
            estimated_delivery: model.estimated_delivery,
            shipped_at: model.shipped_at,
            delivered_at: model.delivered_at,
            created_at: model.created_at,
            updated_at: model.updated_at,
        }
    }
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
pub struct CreateShipmentRequest {
    pub order_id: Uuid,
    #[validate(length(min = 1))]
    pub shipping_address: String,
    #[validate(length(min = 1))]
    pub shipping_method: String,
    #[validate(length(min = 1))]
    pub tracking_number: String,
    #[validate(length(min = 1))]
    pub recipient_name: String,
}

pub async fn list_shipments(
    State(state): State<AppState>,
    Query(query): Query<ShipmentListQuery>,
) -> ApiResult<PaginatedResponse<ShipmentSummary>> {
    let page = query.page.unwrap_or(1).max(1);
    let limit = query.limit.unwrap_or(20).clamp(1, 100);

    // Pass status filter to service for database-side filtering
    let (records, total) = state
        .shipment_service()
        .list_shipments(page, limit, query.status)
        .await?;

    // Convert to summary format (no client-side filtering needed)
    let items: Vec<ShipmentSummary> = records
        .into_iter()
        .map(ShipmentSummary::from)
        .collect();

    let total_pages = (total + limit - 1) / limit;

    Ok(Json(ApiResponse::success(PaginatedResponse {
        items,
        total,
        page,
        limit,
        total_pages,
    })))
}

pub async fn get_shipment(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> ApiResult<ShipmentSummary> {
    match state.shipment_service().get_shipment(id).await? {
        Some(model) => Ok(Json(ApiResponse::success(ShipmentSummary::from(model)))),
        None => Err(ServiceError::NotFound(format!("Shipment {} not found", id))),
    }
}

pub async fn create_shipment(
    State(state): State<AppState>,
    Json(payload): Json<CreateShipmentRequest>,
) -> ApiResult<ShipmentSummary> {
    payload
        .validate()
        .map_err(|e| ServiceError::ValidationError(e.to_string()))?;

    let shipping_method = parse_shipping_method(&payload.shipping_method)?;

    let command = CreateShipmentCommand {
        order_id: payload.order_id,
        shipping_address: payload.shipping_address.clone(),
        shipping_method,
        tracking_number: payload.tracking_number.clone(),
        recipient_name: payload.recipient_name.clone(),
    };

    let shipment_id = state.shipment_service().create_shipment(command).await?;
    let created = state
        .shipment_service()
        .get_shipment(shipment_id)
        .await?
        .ok_or_else(|| ServiceError::NotFound(format!("Shipment {} not found", shipment_id)))?;

    Ok(Json(ApiResponse::success(ShipmentSummary::from(created))))
}

fn parse_shipping_method(value: &str) -> Result<shipment::ShippingMethod, ServiceError> {
    let method = match value.to_ascii_lowercase().as_str() {
        "standard" => shipment::ShippingMethod::Standard,
        "express" => shipment::ShippingMethod::Express,
        "overnight" => shipment::ShippingMethod::Overnight,
        "twoday" | "two-day" | "two_day" => shipment::ShippingMethod::TwoDay,
        "international" => shipment::ShippingMethod::International,
        "custom" => shipment::ShippingMethod::Custom,
        other => {
            return Err(ServiceError::ValidationError(format!(
                "Unsupported shipping method '{}'",
                other
            )))
        }
    };
    Ok(method)
}

pub async fn mark_shipped(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> ApiResult<ShipmentSummary> {
    let updated = state.shipment_service().mark_shipped(id).await?;
    Ok(Json(ApiResponse::success(ShipmentSummary::from(updated))))
}

pub async fn mark_delivered(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> ApiResult<ShipmentSummary> {
    let updated = state.shipment_service().mark_delivered(id).await?;
    Ok(Json(ApiResponse::success(ShipmentSummary::from(updated))))
}

pub async fn track_shipment(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> ApiResult<serde_json::Value> {
    let command = TrackShipmentCommand {
        shipment_id: id,
        circuit_breaker: None,
    };
    let status = state.shipment_service().track_shipment(command).await?;
    Ok(Json(ApiResponse::success(json!({
        "shipment_id": id,
        "status": status
    }))))
}

pub async fn track_by_number(
    State(state): State<AppState>,
    Path(tracking_number): Path<String>,
) -> ApiResult<ShipmentSummary> {
    match state
        .shipment_service()
        .find_by_tracking_number(&tracking_number)
        .await?
    {
        Some(model) => Ok(Json(ApiResponse::success(ShipmentSummary::from(model)))),
        None => Err(ServiceError::NotFound(format!(
            "Shipment with tracking number {} not found",
            tracking_number
        ))),
    }
}
