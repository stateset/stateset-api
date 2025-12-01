use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

use crate::entities::manufacturing::component_serial_number::ComponentStatus;

#[derive(Debug, Deserialize, Validate, Serialize)]
pub struct CreateComponentSerialRequest {
    #[validate(length(min = 1, max = 100))]
    pub serial_number: String,
    #[validate(length(min = 1, max = 100))]
    pub component_type: String,
    #[validate(length(min = 1, max = 100))]
    pub component_sku: String,
    pub supplier_id: Option<Uuid>,
    pub supplier_lot_number: Option<String>,
    pub manufacture_date: Option<NaiveDate>,
    pub receive_date: Option<NaiveDate>,
    pub location: Option<String>,
}

#[derive(Debug, Deserialize, Validate, Serialize)]
pub struct UpdateComponentSerialRequest {
    pub status: Option<ComponentStatus>,
    pub location: Option<String>,
}

#[derive(Debug, Deserialize, Validate, Serialize)]
pub struct InstallComponentRequest {
    pub robot_serial_id: Uuid,
    pub component_serial_id: Uuid,
    pub position: Option<String>,
    pub installed_by: Option<Uuid>,
}

#[derive(Debug, Deserialize, Validate, Serialize)]
pub struct RemoveComponentRequest {
    pub removed_by: Uuid,
    #[validate(length(min = 1, max = 500))]
    pub removal_reason: String,
}

#[derive(Debug, Serialize)]
pub struct ComponentSerialResponse {
    pub id: Uuid,
    pub serial_number: String,
    pub component_type: String,
    pub component_sku: String,
    pub supplier_id: Option<Uuid>,
    pub supplier_lot_number: Option<String>,
    pub manufacture_date: Option<NaiveDate>,
    pub receive_date: Option<NaiveDate>,
    pub status: ComponentStatus,
    pub location: Option<String>,
    pub age_in_days: Option<i64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
