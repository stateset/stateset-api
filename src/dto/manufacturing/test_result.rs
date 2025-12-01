use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use uuid::Uuid;
use validator::Validate;

use crate::entities::manufacturing::test_result::TestStatus;

#[derive(Debug, Deserialize, Validate, Serialize)]
pub struct CreateTestResultRequest {
    pub test_protocol_id: Uuid,
    pub robot_serial_id: Option<Uuid>,
    pub work_order_id: Option<Uuid>,
    pub tested_by: Uuid,
    pub status: TestStatus,
    pub measurements: Option<JsonValue>,
    pub notes: Option<String>,
    pub attachments: Option<JsonValue>,
}

#[derive(Debug, Deserialize, Validate, Serialize)]
pub struct UpdateTestResultRequest {
    pub status: Option<TestStatus>,
    pub measurements: Option<JsonValue>,
    pub notes: Option<String>,
    pub attachments: Option<JsonValue>,
}

#[derive(Debug, Serialize)]
pub struct TestResultResponse {
    pub id: Uuid,
    pub test_protocol_id: Uuid,
    pub test_protocol_name: Option<String>,
    pub robot_serial_id: Option<Uuid>,
    pub robot_serial_number: Option<String>,
    pub work_order_id: Option<Uuid>,
    pub tested_by: Uuid,
    pub test_date: DateTime<Utc>,
    pub status: TestStatus,
    pub measurements: Option<JsonValue>,
    pub notes: Option<String>,
    pub passed: bool,
    pub needs_retest: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct TestResultsSummary {
    pub robot_serial_id: Uuid,
    pub total_tests: i32,
    pub passed: i32,
    pub failed: i32,
    pub pass_rate: f64,
    pub latest_test_date: Option<DateTime<Utc>>,
}
