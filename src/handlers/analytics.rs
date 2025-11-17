use axum::{
    extract::{Query, State},
    response::Json,
    routing::get,
    Router,
};
use serde::Deserialize;
use utoipa::IntoParams;

use crate::{
    errors::ServiceError,
    services::analytics::{
        AnalyticsService, CartMetrics, DashboardMetrics, InventoryMetrics, SalesMetrics,
        SalesTrendPoint, ShipmentMetrics,
    },
    ApiResponse, AppState,
};

/// Build the analytics Router scoped under `/api/v1/analytics`.
pub fn analytics_routes() -> Router<AppState> {
    Router::new()
        .route("/dashboard", get(get_dashboard_metrics))
        .route("/sales", get(get_sales_metrics))
        .route("/sales/trends", get(get_sales_trends))
        .route("/inventory", get(get_inventory_metrics))
        .route("/shipments", get(get_shipment_metrics))
        .route("/carts", get(get_cart_metrics))
}

/// Query parameters for sales trends
#[derive(Debug, Deserialize, IntoParams)]
pub struct SalesTrendsQuery {
    /// Number of days to look back (default: 30)
    #[param(minimum = 1, maximum = 365)]
    pub days: Option<i32>,
    /// Optional status filter (e.g., "completed", "pending")
    #[param(value_type = Option<String>)]
    pub status: Option<String>,
}

/// Analytics handler for business intelligence endpoints
#[utoipa::path(
    get,
    path = "/api/v1/analytics/dashboard",
    responses(
        (status = 200, description = "Dashboard metrics retrieved successfully", body = ApiResponse<DashboardMetrics>)
    ),
    tag = "Analytics"
)]
pub async fn get_dashboard_metrics(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<DashboardMetrics>>, ServiceError> {
    let analytics_service = AnalyticsService::new(state.db);
    let metrics = analytics_service.get_dashboard_metrics().await?;

    Ok(Json(ApiResponse::success(metrics)))
}

/// Get sales trends over time
#[utoipa::path(
    get,
    path = "/api/v1/analytics/sales/trends",
    params(SalesTrendsQuery),
    responses(
        (status = 200, description = "Sales trends retrieved successfully", body = ApiResponse<Vec<SalesTrendPoint>>),
        (status = 400, description = "Invalid trend window", body = crate::errors::ErrorResponse)
    ),
    tag = "Analytics"
)]
pub async fn get_sales_trends(
    State(state): State<AppState>,
    Query(params): Query<SalesTrendsQuery>,
) -> Result<Json<ApiResponse<Vec<SalesTrendPoint>>>, ServiceError> {
    let analytics_service = AnalyticsService::new(state.db);
    let days = params.days.unwrap_or(30);

    if days <= 0 || days > 365 {
        return Err(ServiceError::ValidationError(
            "Days must be between 1 and 365".to_string(),
        ));
    }

    let status = params
        .status
        .as_ref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());
    let trends = analytics_service.get_sales_trends(days, status).await?;
    Ok(Json(ApiResponse::success(trends)))
}

/// Get sales metrics only
#[utoipa::path(
    get,
    path = "/api/v1/analytics/sales",
    responses(
        (status = 200, description = "Sales metrics retrieved successfully", body = ApiResponse<SalesMetrics>)
    ),
    tag = "Analytics"
)]
pub async fn get_sales_metrics(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<SalesMetrics>>, ServiceError> {
    let analytics_service = AnalyticsService::new(state.db);
    let metrics = analytics_service.get_sales_metrics().await?;

    Ok(Json(ApiResponse::success(metrics)))
}

/// Query parameters for inventory metrics
#[derive(Debug, Deserialize, IntoParams)]
pub struct InventoryMetricsQuery {
    /// Threshold below which inventory is considered low stock (default: 10)
    #[param(default = 10, minimum = 1)]
    pub low_stock_threshold: Option<i32>,
}

impl Default for InventoryMetricsQuery {
    fn default() -> Self {
        Self {
            low_stock_threshold: Some(10),
        }
    }
}

/// Get inventory metrics only
#[utoipa::path(
    get,
    path = "/api/v1/analytics/inventory",
    params(InventoryMetricsQuery),
    responses(
        (status = 200, description = "Inventory metrics retrieved successfully", body = ApiResponse<InventoryMetrics>)
    ),
    tag = "Analytics"
)]
pub async fn get_inventory_metrics(
    State(state): State<AppState>,
    Query(params): Query<InventoryMetricsQuery>,
) -> Result<Json<ApiResponse<InventoryMetrics>>, ServiceError> {
    let analytics_service = AnalyticsService::new(state.db);
    let threshold = params
        .low_stock_threshold
        .filter(|value| *value > 0)
        .unwrap_or(10);

    let metrics = analytics_service.get_inventory_metrics(threshold).await?;

    Ok(Json(ApiResponse::success(metrics)))
}

/// Get shipment metrics only
#[utoipa::path(
    get,
    path = "/api/v1/analytics/shipments",
    responses(
        (status = 200, description = "Shipment metrics retrieved successfully", body = ApiResponse<ShipmentMetrics>)
    ),
    tag = "Analytics"
)]
pub async fn get_shipment_metrics(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<ShipmentMetrics>>, ServiceError> {
    let analytics_service = AnalyticsService::new(state.db);
    let metrics = analytics_service.get_shipment_metrics().await?;

    Ok(Json(ApiResponse::success(metrics)))
}

/// Get cart metrics only
#[utoipa::path(
    get,
    path = "/api/v1/analytics/carts",
    responses(
        (status = 200, description = "Cart metrics retrieved successfully", body = ApiResponse<CartMetrics>)
    ),
    tag = "Analytics"
)]
pub async fn get_cart_metrics(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<CartMetrics>>, ServiceError> {
    let analytics_service = AnalyticsService::new(state.db);
    let metrics = analytics_service.get_cart_metrics().await?;

    Ok(Json(ApiResponse::success(metrics)))
}
