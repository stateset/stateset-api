use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use uuid::Uuid;
use validator::Validate;

use crate::entities::manufacturing::robot_service_history::{ServiceStatus, ServiceType};

#[derive(Debug, Deserialize, Validate, Serialize)]
pub struct CreateServiceRecordRequest {
    pub robot_serial_id: Uuid,
    #[validate(length(min = 1, max = 50))]
    pub service_ticket_number: String,
    pub service_type: ServiceType,
    pub service_date: NaiveDate,
    pub technician_id: Option<Uuid>,
    pub description: Option<String>,
    pub scheduled: Option<bool>,
}

#[derive(Debug, Deserialize, Validate, Serialize)]
pub struct UpdateServiceRecordRequest {
    pub work_performed: Option<String>,
    pub parts_replaced: Option<JsonValue>,
    pub labor_hours: Option<Decimal>,
    pub service_cost: Option<Decimal>,
    pub next_service_due: Option<NaiveDate>,
    pub status: Option<ServiceStatus>,
}

#[derive(Debug, Deserialize, Validate, Serialize)]
pub struct CompleteServiceRequest {
    #[validate(length(min = 1))]
    pub work_performed: String,
    pub parts_replaced: Option<JsonValue>,
    pub labor_hours: Option<Decimal>,
    pub service_cost: Option<Decimal>,
    pub next_service_due: Option<NaiveDate>,
}

#[derive(Debug, Serialize)]
pub struct ServiceRecordResponse {
    pub id: Uuid,
    pub robot_serial_id: Uuid,
    pub robot_serial_number: Option<String>,
    pub service_ticket_number: String,
    pub service_type: ServiceType,
    pub service_date: NaiveDate,
    pub technician_id: Option<Uuid>,
    pub description: Option<String>,
    pub work_performed: Option<String>,
    pub parts_replaced: Option<JsonValue>,
    pub labor_hours: Option<Decimal>,
    pub service_cost: Option<Decimal>,
    pub next_service_due: Option<NaiveDate>,
    pub status: ServiceStatus,
    pub is_overdue: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct ServiceHistorySummary {
    pub robot_serial_id: Uuid,
    pub total_services: i32,
    pub last_service_date: Option<NaiveDate>,
    pub next_service_due: Option<NaiveDate>,
    pub total_service_cost: Decimal,
    pub total_labor_hours: Decimal,
}
