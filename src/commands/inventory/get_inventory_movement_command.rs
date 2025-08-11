use crate::{commands::Command, errors::AppError};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

#[derive(Debug, Deserialize, Serialize, Validate)]
pub struct GetInventoryMovementCommand {
    pub product_id: String,
    pub start_date: Option<chrono::NaiveDate>,
    pub end_date: Option<chrono::NaiveDate>,
    pub location_id: Option<String>,
}

impl GetInventoryMovementCommand {
    pub async fn execute(&self) -> Result<(), AppError> {
        // Implementation would go here
        Ok(())
    }
}
