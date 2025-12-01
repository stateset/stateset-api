use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

#[derive(Debug, Deserialize, Validate, Serialize)]
pub struct CreateProductionMetricsRequest {
    pub production_date: NaiveDate,
    pub shift: Option<String>,
    pub production_line_id: Option<Uuid>,
    pub product_id: Option<Uuid>,
    pub robot_model: Option<String>,
    pub planned_quantity: Option<i32>,
    pub actual_quantity: Option<i32>,
    pub quantity_passed: Option<i32>,
    pub quantity_failed: Option<i32>,
    pub quantity_rework: Option<i32>,
    pub planned_hours: Option<Decimal>,
    pub actual_hours: Option<Decimal>,
    pub downtime_hours: Option<Decimal>,
    pub downtime_reason: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ProductionMetricsResponse {
    pub id: Uuid,
    pub production_date: NaiveDate,
    pub shift: Option<String>,
    pub production_line_id: Option<Uuid>,
    pub robot_model: Option<String>,
    pub planned_quantity: Option<i32>,
    pub actual_quantity: Option<i32>,
    pub quantity_passed: Option<i32>,
    pub quantity_failed: Option<i32>,
    pub quantity_rework: Option<i32>,
    pub first_pass_yield: Option<Decimal>,
    pub scrap_rate: Option<Decimal>,
    pub planned_hours: Option<Decimal>,
    pub actual_hours: Option<Decimal>,
    pub downtime_hours: Option<Decimal>,
    pub downtime_reason: Option<String>,
    pub oee: Option<Decimal>,
    pub meets_target_oee: bool,
}

#[derive(Debug, Serialize)]
pub struct ProductionDashboard {
    pub date: NaiveDate,
    pub total_planned: i32,
    pub total_produced: i32,
    pub total_passed: i32,
    pub total_failed: i32,
    pub overall_first_pass_yield: Decimal,
    pub overall_oee: Decimal,
    pub lines: Vec<LineMetrics>,
}

#[derive(Debug, Serialize)]
pub struct LineMetrics {
    pub line_id: Uuid,
    pub line_name: String,
    pub quantity_produced: i32,
    pub first_pass_yield: Decimal,
    pub oee: Decimal,
}

#[derive(Debug, Deserialize)]
pub struct ProductionMetricsQuery {
    pub start_date: Option<NaiveDate>,
    pub end_date: Option<NaiveDate>,
    pub production_line_id: Option<Uuid>,
    pub robot_model: Option<String>,
    pub shift: Option<String>,
}
