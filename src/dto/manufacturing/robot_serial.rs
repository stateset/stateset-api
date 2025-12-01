use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

use crate::entities::manufacturing::robot_serial_number::{RobotStatus, RobotType};

#[derive(Debug, Deserialize, Validate, Serialize)]
pub struct CreateRobotSerialRequest {
    #[validate(length(min = 1, max = 100))]
    pub serial_number: String,
    pub product_id: Uuid,
    pub work_order_id: Option<Uuid>,
    #[validate(length(min = 1, max = 100))]
    pub robot_model: String,
    pub robot_type: RobotType,
    pub manufacturing_date: Option<DateTime<Utc>>,
    pub customer_id: Option<Uuid>,
    pub order_id: Option<Uuid>,
}

#[derive(Debug, Deserialize, Validate, Serialize)]
pub struct UpdateRobotSerialRequest {
    pub status: Option<RobotStatus>,
    pub manufacturing_date: Option<DateTime<Utc>>,
    pub ship_date: Option<DateTime<Utc>>,
    pub customer_id: Option<Uuid>,
    pub order_id: Option<Uuid>,
    pub warranty_start_date: Option<DateTime<Utc>>,
    pub warranty_end_date: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize)]
pub struct RobotSerialResponse {
    pub id: Uuid,
    pub serial_number: String,
    pub product_id: Uuid,
    pub work_order_id: Option<Uuid>,
    pub robot_model: String,
    pub robot_type: RobotType,
    pub manufacturing_date: Option<DateTime<Utc>>,
    pub ship_date: Option<DateTime<Utc>>,
    pub customer_id: Option<Uuid>,
    pub order_id: Option<Uuid>,
    pub status: RobotStatus,
    pub warranty_start_date: Option<DateTime<Utc>>,
    pub warranty_end_date: Option<DateTime<Utc>>,
    pub is_under_warranty: bool,
    pub warranty_remaining_days: Option<i64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct RobotGenealogyResponse {
    pub robot_serial_number: String,
    pub robot_model: String,
    pub robot_status: RobotStatus,
    pub components: Vec<ComponentInRobot>,
}

#[derive(Debug, Serialize)]
pub struct ComponentInRobot {
    pub component_serial_number: String,
    pub component_type: String,
    pub component_sku: String,
    pub position: Option<String>,
    pub installed_at: DateTime<Utc>,
    pub supplier_lot_number: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ListRobotSerialsQuery {
    pub status: Option<RobotStatus>,
    pub robot_type: Option<RobotType>,
    pub robot_model: Option<String>,
    pub customer_id: Option<Uuid>,
    pub limit: Option<u64>,
    pub offset: Option<u64>,
}
