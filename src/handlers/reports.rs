use axum::{
    routing::get,
    extract::{State, Path, Query},
    Router,
};
use std::sync::Arc;
use uuid::Uuid;
use crate::{
    auth::AuthenticatedUser,
    errors::{ApiError, ServiceError},
    services::reports::ReportService,
    main::AppState,
};
use serde::{Serialize, Deserialize};
use validator::Validate;
use chrono::{NaiveDateTime, NaiveDate};
use tracing::info;
use super::common::{validate_input, map_service_error, success_response};

// Request and response DTOs

#[derive(Debug, Deserialize, Validate)]
pub struct DateRangeParams {
    #[validate]
    pub start_date: String,
    #[validate]
    pub end_date: String,
}

impl DateRangeParams {
    /// Converts string dates to NaiveDateTime
    pub fn to_datetime_range(&self) -> Result<(NaiveDateTime, NaiveDateTime), ApiError> {
        let start_date = NaiveDate::parse_from_str(&self.start_date, "%Y-%m-%d")
            .map_err(|e| ApiError::BadRequest(format\!("Invalid start date format: {}", e)))?;
        
        let end_date = NaiveDate::parse_from_str(&self.end_date, "%Y-%m-%d")
            .map_err(|e| ApiError::BadRequest(format\!("Invalid end date format: {}", e)))?;
        
        Ok((
            start_date.and_hms_opt(0, 0, 0).unwrap(),
            end_date.and_hms_opt(23, 59, 59).unwrap(),
        ))
    }
}

// Handler functions

/// Generate order summary report
async fn generate_order_summary_report(
    State(state): State<Arc<AppState>>,
    Query(params): Query<DateRangeParams>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    validate_input(&params)?;
    
    let (start_date, end_date) = params.to_datetime_range()?;
    
    let report = state.services.reports
        .generate_order_summary_report(start_date, end_date)
        .await
        .map_err(map_service_error)?;
    
    info\!("Generated order summary report for period: {}", report.period);
    
    success_response(report)
}

/// Generate inventory report
async fn generate_inventory_report(
    State(state): State<Arc<AppState>>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    let report = state.services.reports
        .generate_inventory_report()
        .await
        .map_err(map_service_error)?;
    
    info\!("Generated inventory report");
    
    success_response(report)
}

/// Generate supplier performance report
async fn generate_supplier_performance_report(
    State(state): State<Arc<AppState>>,
    Path(supplier_id): Path<Uuid>,
    Query(params): Query<DateRangeParams>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    validate_input(&params)?;
    
    let (start_date, end_date) = params.to_datetime_range()?;
    
    let report = state.services.reports
        .generate_supplier_performance_report(&supplier_id, start_date, end_date)
        .await
        .map_err(map_service_error)?;
    
    info\!("Generated supplier performance report for supplier: {}", supplier_id);
    
    success_response(report)
}

/// Generate returns analysis report
async fn generate_returns_report(
    State(state): State<Arc<AppState>>,
    Query(params): Query<DateRangeParams>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    validate_input(&params)?;
    
    let (start_date, end_date) = params.to_datetime_range()?;
    
    let report = state.services.reports
        .generate_returns_report(start_date, end_date)
        .await
        .map_err(map_service_error)?;
    
    info\!("Generated returns analysis report");
    
    success_response(report)
}

/// Generate warehouse efficiency report
async fn generate_warehouse_efficiency_report(
    State(state): State<Arc<AppState>>,
    Path(warehouse_id): Path<Uuid>,
    Query(params): Query<DateRangeParams>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    validate_input(&params)?;
    
    let (start_date, end_date) = params.to_datetime_range()?;
    
    let report = state.services.reports
        .generate_warehouse_efficiency_report(&warehouse_id, start_date, end_date)
        .await
        .map_err(map_service_error)?;
    
    info\!("Generated warehouse efficiency report for warehouse: {}", warehouse_id);
    
    success_response(report)
}

/// Creates the router for report endpoints
pub fn report_routes() -> Router {
    Router::new()
        .route("/orders", get(generate_order_summary_report))
        .route("/inventory", get(generate_inventory_report))
        .route("/suppliers/:supplier_id", get(generate_supplier_performance_report))
        .route("/returns", get(generate_returns_report))
        .route("/warehouses/:warehouse_id", get(generate_warehouse_efficiency_report))
}
