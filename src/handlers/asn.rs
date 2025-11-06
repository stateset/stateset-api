use super::common::{created_response, map_service_error, success_response, validate_input};
use crate::{
    auth::AuthenticatedUser,
    commands::advancedshippingnotice::{
        create_asn_command::{
            CarrierDetails as CreateCarrierDetails, CreateASNCommand,
            CreateASNItemRequest as CommandAsnItem, DimensionUnit as CommandDimensionUnit,
            Dimensions as CommandDimensions, Package as CommandPackage,
            ShippingAddress as CommandShippingAddress, WeightUnit as CommandWeightUnit,
        },
        mark_asn_delivered_command::MarkASNDeliveredCommand,
        mark_asn_in_transit_command::{
            CarrierDetails as TransitCarrierDetails, MarkASNInTransitCommand,
        },
    },
    errors::ApiError,
    handlers::AppState,
    models::asn_entity::{self, ASNStatus},
    PaginatedResponse,
};
use axum::{
    extract::{Path, Query, State},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;
use validator::Validate;

/// Router for ASN endpoints
pub fn asn_routes() -> Router<AppState> {
    Router::new()
        .route("/", post(create_asn))
        .route("/", get(list_asns))
        .route("/{id}", get(get_asn))
        .route("/{id}/in-transit", post(mark_in_transit))
        .route("/{id}/delivered", post(mark_delivered))
}

#[derive(Debug, Serialize, Deserialize, Validate, ToSchema)]
pub struct CreateAsnRequest {
    pub purchase_order_id: Uuid,
    pub supplier_id: Uuid,
    #[validate(length(min = 1))]
    pub supplier_name: String,
    pub expected_delivery_date: Option<String>,
    #[validate]
    pub shipping_address: ShippingAddressRequest,
    #[validate]
    pub carrier: CarrierDetailsRequest,
    #[validate(length(min = 1))]
    pub items: Vec<AsnItemRequest>,
    #[serde(default)]
    #[validate]
    pub packages: Vec<PackageRequest>,
    pub notes: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Validate, Clone, ToSchema)]
pub struct ShippingAddressRequest {
    #[validate(length(min = 1))]
    pub street: String,
    #[validate(length(min = 1))]
    pub city: String,
    #[validate(length(min = 1))]
    pub state: String,
    #[validate(length(min = 1))]
    pub postal_code: String,
    #[validate(length(min = 2))]
    pub country: String,
}

#[derive(Debug, Serialize, Deserialize, Validate, Clone, ToSchema)]
pub struct CarrierDetailsRequest {
    #[validate(length(min = 1))]
    pub carrier_name: String,
    pub tracking_number: Option<String>,
    pub service_level: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Validate, Clone, ToSchema)]
pub struct AsnItemRequest {
    pub product_id: Uuid,
    #[validate(length(min = 1))]
    pub product_name: String,
    #[validate(length(min = 1))]
    pub product_sku: String,
    #[validate(range(min = 1))]
    pub quantity: i32,
    #[validate(range(min = 0.0))]
    pub unit_price: Option<f64>,
}

#[derive(Debug, Serialize, Deserialize, Validate, Clone, ToSchema)]
pub struct PackageRequest {
    #[validate(length(min = 1))]
    pub package_number: String,
    #[validate(range(min = 0.0))]
    pub weight: f64,
    #[validate(length(min = 1))]
    pub weight_unit: String,
    #[validate]
    pub dimensions: Option<PackageDimensionsRequest>,
}

#[derive(Debug, Serialize, Deserialize, Validate, Clone, ToSchema)]
pub struct PackageDimensionsRequest {
    #[validate(range(min = 0.0))]
    pub length: f64,
    #[validate(range(min = 0.0))]
    pub width: f64,
    #[validate(range(min = 0.0))]
    pub height: f64,
    #[validate(length(min = 1))]
    pub unit: String,
}

#[derive(Debug, Serialize, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct AsnListQuery {
    pub page: Option<u64>,
    pub per_page: Option<u64>,
    pub supplier_id: Option<Uuid>,
    pub status: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AsnSummary {
    pub id: Uuid,
    pub asn_number: String,
    pub status: String,
    pub supplier_id: Uuid,
    pub supplier_name: String,
    pub expected_delivery_date: Option<DateTime<Utc>>,
    pub shipping_address: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<asn_entity::Model> for AsnSummary {
    fn from(model: asn_entity::Model) -> Self {
        Self {
            id: model.id,
            asn_number: model.asn_number,
            status: model.status.to_string(),
            supplier_id: model.supplier_id,
            supplier_name: model.supplier_name,
            expected_delivery_date: model.expected_delivery_date,
            shipping_address: model.shipping_address,
            created_at: model.created_at,
            updated_at: model.updated_at,
        }
    }
}

/// Create a new ASN
#[utoipa::path(
    post,
    path = "/api/v1/asns",
    request_body = CreateAsnRequest,
    responses(
        (status = 201, description = "ASN created", body = crate::ApiResponse<AsnSummary>),
        (status = 400, description = "Invalid request", body = crate::errors::ErrorResponse)
    ),
    tag = "asns"
)]
pub async fn create_asn(
    State(state): State<AppState>,
    _user: AuthenticatedUser,
    Json(payload): Json<CreateAsnRequest>,
) -> Result<impl IntoResponse, ApiError> {
    validate_input(&payload)?;

    let command = build_create_command(payload)?;
    let asn_id = state
        .services
        .asn
        .create_asn(command)
        .await
        .map_err(map_service_error)?;

    let asn = state
        .services
        .asn
        .get_asn(&asn_id)
        .await
        .map_err(map_service_error)?
        .ok_or_else(|| ApiError::NotFound(format!("ASN {} not found after creation", asn_id)))?;

    Ok(created_response(AsnSummary::from(asn)))
}

/// Retrieve an ASN by id
#[utoipa::path(
    get,
    path = "/api/v1/asns/{id}",
    params(
        ("id" = Uuid, Path, description = "ASN ID")
    ),
    responses(
        (status = 200, description = "ASN fetched", body = crate::ApiResponse<AsnSummary>),
        (status = 404, description = "ASN not found", body = crate::errors::ErrorResponse)
    ),
    tag = "asns"
)]
pub async fn get_asn(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, ApiError> {
    let asn = state
        .services
        .asn
        .get_asn(&id)
        .await
        .map_err(map_service_error)?
        .ok_or_else(|| ApiError::NotFound(format!("ASN {} not found", id)))?;

    Ok(success_response(AsnSummary::from(asn)))
}

/// List ASNs with optional filters
#[utoipa::path(
    get,
    path = "/api/v1/asns",
    params(AsnListQuery),
    responses(
        (status = 200, description = "ASNs listed", body = crate::ApiResponse<PaginatedResponse<AsnSummary>>),
        (status = 401, description = "Unauthorized", body = crate::errors::ErrorResponse),
        (status = 403, description = "Forbidden", body = crate::errors::ErrorResponse)
    ),
    tag = "asns"
)]
pub async fn list_asns(
    State(state): State<AppState>,
    Query(query): Query<AsnListQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let page = query.page.unwrap_or(1).max(1);
    let per_page = query.per_page.unwrap_or(20).clamp(1, 100);

    let status = match query.status.as_deref() {
        Some(value) => Some(parse_status(value)?),
        None => None,
    };

    let (records, total) = state
        .services
        .asn
        .list_asns(page, per_page, query.supplier_id, status)
        .await
        .map_err(map_service_error)?;

    let items: Vec<AsnSummary> = records.into_iter().map(AsnSummary::from).collect();
    let total_pages = (total + per_page - 1) / per_page;

    Ok(success_response(PaginatedResponse {
        items,
        total,
        page,
        limit: per_page,
        total_pages,
    }))
}

/// Mark an ASN as in transit
#[utoipa::path(
    post,
    path = "/api/v1/asns/{id}/in-transit",
    request_body = MarkAsnInTransitRequest,
    params(
        ("id" = Uuid, Path, description = "ASN ID")
    ),
    responses(
        (status = 200, description = "ASN marked as in transit", body = crate::ApiResponse<AsnSummary>),
        (status = 400, description = "Invalid request", body = crate::errors::ErrorResponse),
        (status = 404, description = "ASN not found", body = crate::errors::ErrorResponse)
    ),
    tag = "asns"
)]
pub async fn mark_in_transit(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(payload): Json<MarkAsnInTransitRequest>,
) -> Result<impl IntoResponse, ApiError> {
    validate_input(&payload)?;

    let MarkAsnInTransitRequest {
        version,
        departure_time,
        estimated_delivery,
        carrier,
    } = payload;

    let tracking_number = carrier.tracking_number.clone().ok_or_else(|| {
        ApiError::ValidationError("tracking_number is required for ASN in-transit status".into())
    })?;

    let command = MarkASNInTransitCommand {
        asn_id: id,
        version,
        carrier_details: TransitCarrierDetails {
            carrier_name: carrier.carrier_name,
            tracking_number,
            service_level: carrier.service_level,
        },
        departure_time,
        estimated_delivery,
    };

    state
        .services
        .asn
        .mark_asn_in_transit(command)
        .await
        .map_err(map_service_error)?;

    let asn = state
        .services
        .asn
        .get_asn(&id)
        .await
        .map_err(map_service_error)?
        .ok_or_else(|| ApiError::NotFound(format!("ASN {} not found", id)))?;

    Ok(success_response(AsnSummary::from(asn)))
}

/// Mark an ASN as delivered
#[utoipa::path(
    post,
    path = "/api/v1/asns/{id}/delivered",
    request_body = MarkAsnDeliveredRequest,
    params(
        ("id" = Uuid, Path, description = "ASN ID")
    ),
    responses(
        (status = 200, description = "ASN marked as delivered", body = crate::ApiResponse<AsnSummary>),
        (status = 400, description = "Invalid request", body = crate::errors::ErrorResponse),
        (status = 404, description = "ASN not found", body = crate::errors::ErrorResponse)
    ),
    tag = "asns"
)]
pub async fn mark_delivered(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(payload): Json<MarkAsnDeliveredRequest>,
) -> Result<impl IntoResponse, ApiError> {
    validate_input(&payload)?;

    let command = MarkASNDeliveredCommand {
        asn_id: id,
        version: payload.version,
        delivery_date: payload.delivery_date,
        recipient_name: payload.recipient_name,
        delivery_notes: payload.delivery_notes,
        proof_of_delivery: payload.proof_of_delivery,
    };

    state
        .services
        .asn
        .mark_asn_delivered(command)
        .await
        .map_err(map_service_error)?;

    let asn = state
        .services
        .asn
        .get_asn(&id)
        .await
        .map_err(map_service_error)?
        .ok_or_else(|| ApiError::NotFound(format!("ASN {} not found", id)))?;

    Ok(success_response(AsnSummary::from(asn)))
}

#[derive(Debug, Serialize, Deserialize, Validate, ToSchema)]
pub struct MarkAsnInTransitRequest {
    pub version: i32,
    pub departure_time: DateTime<Utc>,
    pub estimated_delivery: DateTime<Utc>,
    #[validate]
    pub carrier: CarrierDetailsRequest,
}

#[derive(Debug, Serialize, Deserialize, Validate, ToSchema)]
pub struct MarkAsnDeliveredRequest {
    pub version: i32,
    pub delivery_date: DateTime<Utc>,
    #[validate(length(min = 1))]
    pub recipient_name: String,
    pub delivery_notes: Option<String>,
    pub proof_of_delivery: Option<String>,
}

fn build_create_command(payload: CreateAsnRequest) -> Result<CreateASNCommand, ApiError> {
    let expected_delivery_date = match payload.expected_delivery_date {
        Some(value) => Some(parse_datetime(&value)?),
        None => None,
    };

    let shipping_address = CommandShippingAddress {
        street: payload.shipping_address.street,
        city: payload.shipping_address.city,
        state: payload.shipping_address.state,
        postal_code: payload.shipping_address.postal_code,
        country: payload.shipping_address.country,
    };

    let carrier_details = CreateCarrierDetails {
        carrier_name: payload.carrier.carrier_name,
        tracking_number: payload.carrier.tracking_number,
        service_level: payload.carrier.service_level,
    };

    let items = payload
        .items
        .into_iter()
        .map(|item| CommandAsnItem {
            product_id: item.product_id,
            product_name: item.product_name,
            product_sku: item.product_sku,
            quantity: item.quantity,
            unit_price: item.unit_price,
        })
        .collect();

    let packages = payload
        .packages
        .into_iter()
        .map(|pkg| {
            let weight_unit =
                parse_weight_unit(&pkg.weight_unit).map_err(ApiError::ValidationError)?;

            let dimensions = if let Some(dims) = pkg.dimensions {
                let unit = parse_dimension_unit(&dims.unit).map_err(ApiError::ValidationError)?;
                Some(CommandDimensions {
                    length: dims.length,
                    width: dims.width,
                    height: dims.height,
                    unit,
                })
            } else {
                None
            };

            Ok(CommandPackage {
                package_number: pkg.package_number,
                weight: pkg.weight,
                weight_unit,
                dimensions,
            })
        })
        .collect::<Result<Vec<_>, ApiError>>()?;

    Ok(CreateASNCommand {
        purchase_order_id: payload.purchase_order_id,
        supplier_id: payload.supplier_id,
        supplier_name: payload.supplier_name,
        expected_delivery_date,
        shipping_address,
        carrier_details,
        items,
        packages,
    })
}

fn parse_status(value: &str) -> Result<ASNStatus, ApiError> {
    match value.to_ascii_lowercase().as_str() {
        "draft" => Ok(ASNStatus::Draft),
        "submitted" => Ok(ASNStatus::Submitted),
        "intransit" | "in_transit" => Ok(ASNStatus::InTransit),
        "delivered" => Ok(ASNStatus::Delivered),
        "completed" => Ok(ASNStatus::Completed),
        "cancelled" | "canceled" => Ok(ASNStatus::Cancelled),
        "onhold" | "on_hold" => Ok(ASNStatus::OnHold),
        other => Err(ApiError::ValidationError(format!(
            "Unknown ASN status '{}'",
            other
        ))),
    }
}

fn parse_datetime(value: &str) -> Result<DateTime<Utc>, ApiError> {
    DateTime::parse_from_rfc3339(value)
        .map(|dt| dt.with_timezone(&Utc))
        .or_else(|_| {
            DateTime::parse_from_str(&format!("{}T00:00:00Z", value), "%Y-%m-%dT%H:%M:%SZ")
                .map(|dt| dt.with_timezone(&Utc))
        })
        .map_err(|_| ApiError::ValidationError(format!("Invalid datetime format: {}", value)))
}

fn parse_weight_unit(value: &str) -> Result<CommandWeightUnit, String> {
    match value.to_ascii_uppercase().as_str() {
        "KG" => Ok(CommandWeightUnit::KG),
        "LB" | "LBS" => Ok(CommandWeightUnit::LB),
        other => Err(format!("Unsupported weight unit '{}'", other)),
    }
}

fn parse_dimension_unit(value: &str) -> Result<CommandDimensionUnit, String> {
    match value.to_ascii_uppercase().as_str() {
        "CM" => Ok(CommandDimensionUnit::CM),
        "IN" => Ok(CommandDimensionUnit::IN),
        other => Err(format!("Unsupported dimension unit '{}'", other)),
    }
}
