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
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;
use validator::Validate;

#[derive(Debug, Deserialize, Default, ToSchema, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ShipmentListQuery {
    pub page: Option<u64>,
    pub limit: Option<u64>,
    pub status: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(example = json!({
    "id": "990e8400-e29b-41d4-a716-446655440000",
    "order_id": "550e8400-e29b-41d4-a716-446655440000",
    "tracking_number": "1Z999AA10123456784",
    "status": "in_transit",
    "shipping_method": "express",
    "shipping_address": "123 Main Street, San Francisco, CA 94102, US",
    "recipient_name": "John Doe",
    "estimated_delivery": "2024-12-12T18:00:00Z",
    "shipped_at": "2024-12-09T14:30:00Z",
    "delivered_at": null,
    "created_at": "2024-12-09T10:30:00Z",
    "updated_at": "2024-12-09T14:30:00Z"
}))]
pub struct ShipmentSummary {
    /// Shipment UUID
    #[schema(example = "990e8400-e29b-41d4-a716-446655440000")]
    pub id: Uuid,
    /// Associated order UUID
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub order_id: Uuid,
    /// Carrier tracking number
    #[schema(example = "1Z999AA10123456784")]
    pub tracking_number: String,
    /// Shipment status (pending, label_created, in_transit, out_for_delivery, delivered, exception)
    #[schema(example = "in_transit")]
    pub status: String,
    /// Shipping method (standard, express, overnight, twoday, international, custom)
    #[schema(example = "express")]
    pub shipping_method: String,
    /// Full shipping address
    #[schema(example = "123 Main Street, San Francisco, CA 94102, US")]
    pub shipping_address: String,
    /// Recipient name
    #[schema(example = "John Doe")]
    pub recipient_name: String,
    /// Estimated delivery date
    pub estimated_delivery: Option<DateTime<Utc>>,
    /// Actual ship date
    pub shipped_at: Option<DateTime<Utc>>,
    /// Actual delivery date
    pub delivered_at: Option<DateTime<Utc>>,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
    /// Last update timestamp
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
#[schema(example = json!({
    "order_id": "550e8400-e29b-41d4-a716-446655440000",
    "shipping_address": "123 Main Street, San Francisco, CA 94102, US",
    "shipping_method": "express",
    "tracking_number": "1Z999AA10123456784",
    "recipient_name": "John Doe"
}))]
pub struct CreateShipmentRequest {
    /// Order UUID to ship
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub order_id: Uuid,
    /// Full shipping address
    #[validate(length(min = 1))]
    #[schema(example = "123 Main Street, San Francisco, CA 94102, US")]
    pub shipping_address: String,
    /// Shipping method (standard, express, overnight, twoday, international, custom)
    #[validate(length(min = 1))]
    #[schema(example = "express")]
    pub shipping_method: String,
    /// Carrier tracking number
    #[validate(length(min = 1))]
    #[schema(example = "1Z999AA10123456784")]
    pub tracking_number: String,
    /// Recipient name
    #[validate(length(min = 1))]
    #[schema(example = "John Doe")]
    pub recipient_name: String,
}

#[utoipa::path(
    get,
    path = "/api/v1/shipments",
    params(ShipmentListQuery),
    responses(
        (status = 200, description = "Shipments listed", body = ApiResponse<PaginatedResponse<ShipmentSummary>>),
        (status = 401, description = "Unauthorized", body = crate::errors::ErrorResponse),
        (status = 403, description = "Forbidden", body = crate::errors::ErrorResponse)
    ),
    tag = "shipments"
)]
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
    let items: Vec<ShipmentSummary> = records.into_iter().map(ShipmentSummary::from).collect();

    let total_pages = (total + limit - 1) / limit;

    Ok(Json(ApiResponse::success(PaginatedResponse {
        items,
        total,
        page,
        limit,
        total_pages,
    })))
}

#[utoipa::path(
    get,
    path = "/api/v1/shipments/:id",
    params(
        ("id" = Uuid, Path, description = "Shipment ID")
    ),
    responses(
        (status = 200, description = "Shipment fetched", body = ApiResponse<ShipmentSummary>),
        (status = 404, description = "Shipment not found", body = crate::errors::ErrorResponse)
    ),
    tag = "shipments"
)]
pub async fn get_shipment(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> ApiResult<ShipmentSummary> {
    match state.shipment_service().get_shipment(id).await? {
        Some(model) => Ok(Json(ApiResponse::success(ShipmentSummary::from(model)))),
        None => Err(ServiceError::NotFound(format!("Shipment {} not found", id))),
    }
}

#[utoipa::path(
    post,
    path = "/api/v1/shipments",
    request_body = CreateShipmentRequest,
    responses(
        (status = 201, description = "Shipment created", body = ApiResponse<ShipmentSummary>),
        (status = 400, description = "Invalid request", body = crate::errors::ErrorResponse)
    ),
    tag = "shipments"
)]
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

#[utoipa::path(
    post,
    path = "/api/v1/shipments/:id/ship",
    params(
        ("id" = Uuid, Path, description = "Shipment ID")
    ),
    responses(
        (status = 200, description = "Shipment marked as shipped", body = ApiResponse<ShipmentSummary>),
        (status = 404, description = "Shipment not found", body = crate::errors::ErrorResponse)
    ),
    tag = "shipments"
)]
pub async fn mark_shipped(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> ApiResult<ShipmentSummary> {
    let updated = state.shipment_service().mark_shipped(id).await?;
    Ok(Json(ApiResponse::success(ShipmentSummary::from(updated))))
}

#[utoipa::path(
    post,
    path = "/api/v1/shipments/:id/deliver",
    params(
        ("id" = Uuid, Path, description = "Shipment ID")
    ),
    responses(
        (status = 200, description = "Shipment marked as delivered", body = ApiResponse<ShipmentSummary>),
        (status = 404, description = "Shipment not found", body = crate::errors::ErrorResponse)
    ),
    tag = "shipments"
)]
pub async fn mark_delivered(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> ApiResult<ShipmentSummary> {
    let updated = state.shipment_service().mark_delivered(id).await?;
    Ok(Json(ApiResponse::success(ShipmentSummary::from(updated))))
}

#[utoipa::path(
    get,
    path = "/api/v1/shipments/:id/track",
    params(
        ("id" = Uuid, Path, description = "Shipment ID")
    ),
    responses(
        (status = 200, description = "Shipment tracking status", body = ApiResponse<serde_json::Value>),
        (status = 404, description = "Shipment not found", body = crate::errors::ErrorResponse)
    ),
    tag = "shipments"
)]
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

#[utoipa::path(
    get,
    path = "/api/v1/shipments/track/:tracking_number}",
    params(
        ("tracking_number" = String, Path, description = "Tracking number")
    ),
    responses(
        (status = 200, description = "Shipment fetched by tracking number", body = ApiResponse<ShipmentSummary>),
        (status = 404, description = "Shipment not found", body = crate::errors::ErrorResponse)
    ),
    tag = "shipments"
)]
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
