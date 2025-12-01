use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use uuid::Uuid;
use validator::Validate;

use crate::entities::manufacturing::test_protocol::{ProtocolStatus, TestType};

#[derive(Debug, Deserialize, Validate, Serialize)]
pub struct CreateTestProtocolRequest {
    #[validate(length(min = 1, max = 50))]
    pub protocol_number: String,
    #[validate(length(min = 1, max = 200))]
    pub name: String,
    pub description: Option<String>,
    pub test_type: TestType,
    pub applicable_models: Option<Vec<String>>,
    pub test_equipment_required: Option<Vec<String>>,
    pub estimated_duration_minutes: Option<i32>,
    pub pass_criteria: Option<JsonValue>,
    pub procedure_steps: Option<JsonValue>,
    pub revision: Option<String>,
}

#[derive(Debug, Deserialize, Validate, Serialize)]
pub struct UpdateTestProtocolRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub pass_criteria: Option<JsonValue>,
    pub procedure_steps: Option<JsonValue>,
    pub revision: Option<String>,
    pub status: Option<ProtocolStatus>,
}

#[derive(Debug, Serialize)]
pub struct TestProtocolResponse {
    pub id: Uuid,
    pub protocol_number: String,
    pub name: String,
    pub description: Option<String>,
    pub test_type: TestType,
    pub applicable_models: Option<Vec<String>>,
    pub test_equipment_required: Option<Vec<String>>,
    pub estimated_duration_minutes: Option<i32>,
    pub pass_criteria: Option<JsonValue>,
    pub procedure_steps: Option<JsonValue>,
    pub revision: Option<String>,
    pub status: ProtocolStatus,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
