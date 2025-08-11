use crate::{commands::Command, errors::AppError};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;
use std::sync::Arc;
use crate::{db::DbPool, events::EventSender};

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

    pub async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<(), AppError> {

        // Implementation would go here

        Ok(())

    }

}
