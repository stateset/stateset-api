use std::sync::Arc;
use sea_orm::*;
use crate::{
    db::DbPool,
    errors::ServiceError,
    events::{Event, EventSender},
    models::inventory_level_entity::{self, Entity as InventoryLevel},
};
use serde::{Deserialize, Serialize};
use tracing::{error, info, instrument};
use uuid::Uuid;
use validator::Validate;
use prometheus::{IntCounter, Counter};
use lazy_static::lazy_static;
use async_trait::async_trait;
use crate::commands::Command;

lazy_static! {
    static ref INVENTORY_LEVELS_SET: IntCounter = 
        IntCounter::new("inventory_levels_set_total", "Total number of inventory level settings")
            .expect("metric can be created");

    static ref INVENTORY_LEVELS_SET_FAILURES: IntCounter = 
        IntCounter::new("inventory_levels_set_failures_total", "Total number of failed inventory level settings")
            .expect("metric can be created");
}

#[derive(Debug, Serialize, Deserialize, Validate, Clone)]
pub struct SetInventoryLevelsCommand {
    pub product_id: Uuid,
    pub location_id: Uuid,
    #[validate(range(min = 0, message = "Quantity cannot be negative"))]
    pub quantity: i32,
    #[validate(range(min = 0, message = "Reserved quantity cannot be negative"))]
    pub reserved: i32,
    #[validate(range(min = 0, message = "Allocated quantity cannot be negative"))]
    pub allocated: i32,
    #[validate(range(min = 0, message = "Available quantity cannot be negative"))]
    pub available: i32,
    pub reason: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SetInventoryLevelsResult {
    pub product_id: Uuid,
    pub location_id: Uuid,
    pub quantity: i32,
    pub reserved: i32,
    pub allocated: i32,
    pub available: i32,
}

#[async_trait]
impl Command for SetInventoryLevelsCommand {
    type Result = SetInventoryLevelsResult;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(
        &self,
        db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        self.validate().map_err(|e| {
            INVENTORY_LEVELS_SET_FAILURES.inc();
            let msg = format!("Invalid input: {}", e);
            error!("{}", msg);
            ServiceError::ValidationError(msg)
        })?;

        let db = db_pool.as_ref();

        // Check if inventory level exists
        let inventory_level = InventoryLevel::find()
            .filter(inventory_level_entity::Column::ProductId.eq(self.product_id))
            .filter(inventory_level_entity::Column::LocationId.eq(self.location_id))
            .one(db)
            .await
            .map_err(|e| {
                INVENTORY_LEVELS_SET_FAILURES.inc();
                let msg = format!("Failed to check inventory level: {}", e);
                error!("{}", msg);
                ServiceError::DatabaseError(msg)
            })?;

        let inventory_level = match inventory_level {
            Some(level) => {
                // Update existing inventory level
                let mut level_model: inventory_level_entity::ActiveModel = level.into();
                level_model.quantity = Set(self.quantity);
                level_model.reserved = Set(self.reserved);
                level_model.allocated = Set(self.allocated);
                level_model.available = Set(self.available);
                level_model.last_updated = Set(chrono::Utc::now().naive_utc());

                level_model.update(db).await.map_err(|e| {
                    INVENTORY_LEVELS_SET_FAILURES.inc();
                    let msg = format!("Failed to update inventory level: {}", e);
                    error!("{}", msg);
                    ServiceError::DatabaseError(msg)
                })?
            },
            None => {
                // Create new inventory level
                let new_level = inventory_level_entity::ActiveModel {
                    id: Set(Uuid::new_v4()),
                    product_id: Set(self.product_id),
                    location_id: Set(self.location_id),
                    quantity: Set(self.quantity),
                    reserved: Set(self.reserved),
                    allocated: Set(self.allocated),
                    available: Set(self.available),
                    created_at: Set(chrono::Utc::now().naive_utc()),
                    last_updated: Set(chrono::Utc::now().naive_utc()),
                    ..Default::default()
                };

                new_level.insert(db).await.map_err(|e| {
                    INVENTORY_LEVELS_SET_FAILURES.inc();
                    let msg = format!("Failed to create inventory level: {}", e);
                    error!("{}", msg);
                    ServiceError::DatabaseError(msg)
                })?
            }
        };

        // Emit inventory level set event
        event_sender
            .send(Event::InventoryLevelSet(self.product_id, self.location_id))
            .await
            .map_err(|e| {
                INVENTORY_LEVELS_SET_FAILURES.inc();
                let msg = format!("Failed to send event for inventory level set: {}", e);
                error!("{}", msg);
                ServiceError::EventError(msg)
            })?;

        info!(
            product_id = %self.product_id,
            location_id = %self.location_id,
            quantity = %self.quantity,
            reserved = %self.reserved,
            allocated = %self.allocated,
            available = %self.available,
            "Inventory levels set successfully"
        );

        INVENTORY_LEVELS_SET.inc();

        // Return result
        Ok(SetInventoryLevelsResult {
            product_id: inventory_level.product_id,
            location_id: inventory_level.location_id,
            quantity: inventory_level.quantity,
            reserved: inventory_level.reserved,
            allocated: inventory_level.allocated,
            available: inventory_level.available,
        })
    }
}