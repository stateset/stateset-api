use async_trait::async_trait;
use serde::{Serialize, Deserialize};
use std::sync::Arc;
use crate::{errors::ServiceError, db::DbPool, models::{OrderItem}};
use crate::events::{Event, EventSender};
use validator::Validate;
use tracing::{info, error, instrument};
use diesel::prelude::*;
use prometheus::IntCounter;

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct AdjustInventoryCommand {
    pub product_id: i32,

    #[validate(range(min = 1))]
    pub quantity: i32,
}

#[async_trait]
impl Command for AdjustInventoryCommand {
    type Result = ();

    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;

        // Adjust the inventory level
        diesel::update(inventory::table.find(self.product_id))
            .set(inventory::quantity.eq(inventory::quantity + self.quantity))
            .execute(&conn)
            .map_err(|e| ServiceError::DatabaseError)?;

        // Trigger an event
        event_sender.send(Event::InventoryAdjusted(self.product_id, self.quantity)).await.map_err(|e| ServiceError::EventError(e.to_string()))?;

        // Log the adjustment
        info!("Inventory adjusted: Product ID: {}, Quantity: {}", self.product_id, self.quantity);

        Ok(())
    }
}