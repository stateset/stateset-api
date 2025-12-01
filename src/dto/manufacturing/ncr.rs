use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

use crate::entities::manufacturing::non_conformance_report::{
    Disposition, IssueType, NcrStatus, Severity,
};

#[derive(Debug, Deserialize, Validate, Serialize)]
pub struct CreateNcrRequest {
    #[validate(length(min = 1, max = 50))]
    pub ncr_number: String,
    pub robot_serial_id: Option<Uuid>,
    pub work_order_id: Option<Uuid>,
    pub component_serial_id: Option<Uuid>,
    pub reported_by: Uuid,
    pub issue_type: IssueType,
    pub severity: Severity,
    #[validate(length(min = 1))]
    pub description: String,
    pub assigned_to: Option<Uuid>,
}

#[derive(Debug, Deserialize, Validate, Serialize)]
pub struct UpdateNcrRequest {
    pub root_cause: Option<String>,
    pub corrective_action: Option<String>,
    pub preventive_action: Option<String>,
    pub assigned_to: Option<Uuid>,
    pub status: Option<NcrStatus>,
    pub disposition: Option<Disposition>,
}

#[derive(Debug, Deserialize, Validate, Serialize)]
pub struct CloseNcrRequest {
    #[validate(length(min = 1))]
    pub resolution_notes: String,
    pub disposition: Disposition,
}

#[derive(Debug, Serialize)]
pub struct NcrResponse {
    pub id: Uuid,
    pub ncr_number: String,
    pub robot_serial_id: Option<Uuid>,
    pub work_order_id: Option<Uuid>,
    pub component_serial_id: Option<Uuid>,
    pub reported_by: Uuid,
    pub reported_at: DateTime<Utc>,
    pub issue_type: IssueType,
    pub severity: Severity,
    pub description: String,
    pub root_cause: Option<String>,
    pub corrective_action: Option<String>,
    pub preventive_action: Option<String>,
    pub assigned_to: Option<Uuid>,
    pub status: NcrStatus,
    pub resolution_date: Option<DateTime<Utc>>,
    pub disposition: Option<Disposition>,
    pub is_open: bool,
    pub is_critical: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct ListNcrQuery {
    pub status: Option<NcrStatus>,
    pub severity: Option<Severity>,
    pub robot_serial_id: Option<Uuid>,
    pub assigned_to: Option<Uuid>,
    pub limit: Option<u64>,
    pub offset: Option<u64>,
}
