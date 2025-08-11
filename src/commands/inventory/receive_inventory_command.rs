use crate::{commands::Command, errors::AppError};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;
use std::sync::Arc;
use crate::{db::DbPool, events::EventSender};

#[derive(Debug, Deserialize, Serialize, Validate)]
pub struct ReceiveInventoryCommand {
    pub product_id: String,
    pub location_id: String,
    pub quantity: i32,
    pub lot_number: Option<String>,
    pub expiration_date: Option<chrono::NaiveDate>,
    pub notes: Option<String>,
}

impl ReceiveInventoryCommand {

    pub async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<(), AppError> {

        // Implementation would go here

        Ok(())

    }

}
