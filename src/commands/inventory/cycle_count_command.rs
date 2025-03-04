use crate::errors::AppError;
use serde::{Deserialize, Serialize};
use validator::Validate;
use uuid::Uuid;

#[derive(Debug, Deserialize, Serialize, Validate)]
pub struct CycleCountItem {
    pub product_id: String,
    pub counted_quantity: i32,
    pub lot_number: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Validate)]
pub struct CycleCountCommand {
    pub location_id: String,
    pub items: Vec<CycleCountItem>,
    pub notes: Option<String>,
    pub counted_by: String,
}

impl CycleCountCommand {
    pub async fn execute(&self) -> Result<(), AppError> {
        // Implementation would go here
        Ok(())
    }
}