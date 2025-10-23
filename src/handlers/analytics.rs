use axum::{extract::Query, response::Json};
use rust_decimal::Decimal;
use serde::Deserialize;

use crate::{errors::ServiceError, services::analytics::AnalyticsService, ApiResponse, AppState};

/// Query parameters for sales trends
#[derive(Debug, Deserialize)]
pub struct SalesTrendsQuery {
    /// Number of days to look back (default: 30)
    pub days: Option<i32>,
}

/// Analytics handler for business intelligence endpoints
pub async fn get_dashboard_metrics(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> Result<Json<ApiResponse<serde_json::Value>>, ServiceError> {
    let analytics_service = AnalyticsService::new(state.db);
    let metrics = analytics_service.get_dashboard_metrics().await?;

    let response = serde_json::json!({
        "sales": {
            "total_orders": metrics.sales.total_orders,
            "total_revenue": metrics.sales.total_revenue,
            "average_order_value": metrics.sales.average_order_value,
            "orders_today": metrics.sales.orders_today,
            "revenue_today": metrics.sales.revenue_today,
            "orders_this_week": metrics.sales.orders_this_week,
            "revenue_this_week": metrics.sales.revenue_this_week,
            "orders_this_month": metrics.sales.orders_this_month,
            "revenue_this_month": metrics.sales.revenue_this_month,
        },
        "inventory": {
            "total_products": metrics.inventory.total_products,
            "low_stock_items": metrics.inventory.low_stock_items,
            "out_of_stock_items": metrics.inventory.out_of_stock_items,
            "total_value": metrics.inventory.total_value,
            "average_stock_level": metrics.inventory.average_stock_level,
        },
        "shipments": {
            "total_shipments": metrics.shipments.total_shipments,
            "pending_shipments": metrics.shipments.pending_shipments,
            "shipped_today": metrics.shipments.shipped_today,
            "delivered_today": metrics.shipments.delivered_today,
            "average_delivery_time_hours": metrics.shipments.average_delivery_time_hours,
        },
        "generated_at": metrics.generated_at,
    });

    Ok(Json(ApiResponse::success(response)))
}

/// Get sales trends over time
pub async fn get_sales_trends(
    axum::extract::State(state): axum::extract::State<AppState>,
    Query(params): Query<SalesTrendsQuery>,
) -> Result<Json<ApiResponse<Vec<(String, Decimal)>>>, ServiceError> {
    let analytics_service = AnalyticsService::new(state.db);
    let days = params.days.unwrap_or(30);

    if days <= 0 || days > 365 {
        return Err(ServiceError::ValidationError(
            "Days must be between 1 and 365".to_string(),
        ));
    }

    let trends = analytics_service.get_sales_trends(days).await?;
    Ok(Json(ApiResponse::success(trends)))
}

/// Get sales metrics only
pub async fn get_sales_metrics(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> Result<Json<ApiResponse<serde_json::Value>>, ServiceError> {
    let analytics_service = AnalyticsService::new(state.db);
    let metrics = analytics_service.get_sales_metrics().await?;

    let response = serde_json::json!({
        "total_orders": metrics.total_orders,
        "total_revenue": metrics.total_revenue,
        "average_order_value": metrics.average_order_value,
        "orders_today": metrics.orders_today,
        "revenue_today": metrics.revenue_today,
        "orders_this_week": metrics.orders_this_week,
        "revenue_this_week": metrics.revenue_this_week,
        "orders_this_month": metrics.orders_this_month,
        "revenue_this_month": metrics.revenue_this_month,
    });

    Ok(Json(ApiResponse::success(response)))
}

/// Get inventory metrics only
pub async fn get_inventory_metrics(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> Result<Json<ApiResponse<serde_json::Value>>, ServiceError> {
    let analytics_service = AnalyticsService::new(state.db);
    let metrics = analytics_service.get_inventory_metrics().await?;

    let response = serde_json::json!({
        "total_products": metrics.total_products,
        "low_stock_items": metrics.low_stock_items,
        "out_of_stock_items": metrics.out_of_stock_items,
        "total_value": metrics.total_value,
        "average_stock_level": metrics.average_stock_level,
    });

    Ok(Json(ApiResponse::success(response)))
}

/// Get shipment metrics only
pub async fn get_shipment_metrics(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> Result<Json<ApiResponse<serde_json::Value>>, ServiceError> {
    let analytics_service = AnalyticsService::new(state.db);
    let metrics = analytics_service.get_shipment_metrics().await?;

    let response = serde_json::json!({
        "total_shipments": metrics.total_shipments,
        "pending_shipments": metrics.pending_shipments,
        "shipped_today": metrics.shipped_today,
        "delivered_today": metrics.delivered_today,
        "average_delivery_time_hours": metrics.average_delivery_time_hours,
    });

    Ok(Json(ApiResponse::success(response)))
}
