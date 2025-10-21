use crate::commands::Command;
use crate::{
    db::DbPool,
    errors::ServiceError,
    events::{Event, EventSender},
    models::inventory_level_entity::{self, Column as InventoryColumn, Entity as InventoryLevel},
};
use async_trait::async_trait;
use chrono::Utc;
use lazy_static::lazy_static;
use prometheus::{Counter, IntCounter};
use sea_orm::{*, Set, TransactionError};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info, instrument};
use uuid::Uuid;
use validator::Validate;

lazy_static! {
    static ref INVENTORY_TRANSFERS: IntCounter = IntCounter::new(
        "inventory_transfers_total",
        "Total number of inventory transfers"
    )
    .expect("metric can be created");
    static ref INVENTORY_TRANSFER_FAILURES: IntCounter = IntCounter::new(
        "inventory_transfer_failures_total",
        "Total number of failed inventory transfers"
    )
    .expect("metric can be created");
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct TransferInventoryCommand {
    #[validate(length(min = 1, message = "Product ID cannot be empty"))]
    pub product_id: String,

    #[validate(length(min = 1, message = "From location ID cannot be empty"))]
    pub from_location_id: String,

    #[validate(length(min = 1, message = "To location ID cannot be empty"))]
    pub to_location_id: String,

    #[validate(range(min = 1, message = "Quantity must be at least 1"))]
    pub quantity: i32,

    pub lot_number: Option<String>,

    pub notes: Option<String>,
}

#[async_trait]
impl Command for TransferInventoryCommand {
    type Result = ();

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(
        &self,
        db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        self.validate().map_err(|e| {
            INVENTORY_TRANSFER_FAILURES.inc();
            let msg = format!("Invalid input: {}", e);
            error!("{}", msg);
            ServiceError::ValidationError(msg)
        })?;

        // Convert string IDs to Uuid
        let product_id = Uuid::parse_str(&self.product_id).map_err(|e| {
            INVENTORY_TRANSFER_FAILURES.inc();
            let msg = format!("Invalid product ID format: {}", e);
            error!("{}", msg);
            ServiceError::ValidationError(msg)
        })?;

        let from_location_id = Uuid::parse_str(&self.from_location_id).map_err(|e| {
            INVENTORY_TRANSFER_FAILURES.inc();
            let msg = format!("Invalid from location ID format: {}", e);
            error!("{}", msg);
            ServiceError::ValidationError(msg)
        })?;

        let to_location_id = Uuid::parse_str(&self.to_location_id).map_err(|e| {
            INVENTORY_TRANSFER_FAILURES.inc();
            let msg = format!("Invalid to location ID format: {}", e);
            error!("{}", msg);
            ServiceError::ValidationError(msg)
        })?;

        // Verify that from_location_id != to_location_id
        if from_location_id == to_location_id {
            INVENTORY_TRANSFER_FAILURES.inc();
            let msg = "Cannot transfer inventory to the same location".to_string();
            error!("{}", msg);
            return Err(ServiceError::ValidationError(msg));
        }

        let db = db_pool.as_ref();

        // Execute the transfer in a transaction
        let result = db
            .transaction::<_, (), ServiceError>(|txn| {
                Box::pin(async move {
                    // 1. Check if there's enough inventory at the source location
                    let source_inventory = InventoryLevel::find()
                        .filter(InventoryColumn::ProductId.eq(product_id))
                        .filter(InventoryColumn::WarehouseId.eq(from_location_id.clone()))
                        .one(txn)
                        .await
                        .map_err(|e| {
                            INVENTORY_TRANSFER_FAILURES.inc();
                            ServiceError::db_error(e)
                        })?
                        .ok_or_else(|| {
                            INVENTORY_TRANSFER_FAILURES.inc();
                            ServiceError::NotFound(format!(
                                "No inventory found for product {} at location {}",
                                product_id, from_location_id
                            ))
                        })?;

                    // Calculate available quantity (on_hand - reserved - allocated)
                    let available_quantity = source_inventory.on_hand_quantity 
                        - source_inventory.reserved_quantity 
                        - source_inventory.allocated_quantity;

                    if available_quantity < self.quantity {
                        INVENTORY_TRANSFER_FAILURES.inc();
                        let msg = format!(
                            "Insufficient inventory: available={}, requested={}",
                            available_quantity, self.quantity
                        );
                        error!("{}", msg);
                        return Err(ServiceError::ValidationError(msg));
                    }

                    // 2. Update source inventory
                    let mut source_inventory: inventory_level_entity::ActiveModel =
                        source_inventory.into();
                    let new_source_quantity =
                        source_inventory.on_hand_quantity.clone().unwrap() - self.quantity;
                    source_inventory.on_hand_quantity = Set(new_source_quantity);
                    source_inventory.updated_at = Set(Utc::now());
                    source_inventory.update(txn).await.map_err(|e| {
                        let msg = format!("Failed to update source inventory: {}", e);
                        error!("{}", msg);
                        ServiceError::db_error(e)
                    })?;

                    // 3. Check if destination inventory exists
                    let dest_inventory = InventoryLevel::find()
                        .filter(InventoryColumn::ProductId.eq(product_id))
                        .filter(InventoryColumn::WarehouseId.eq(to_location_id.clone()))
                        .one(txn)
                        .await
                        .map_err(|e| {
                            INVENTORY_TRANSFER_FAILURES.inc();
                            ServiceError::db_error(e)
                        })?;

                    match dest_inventory {
                        Some(inv) => {
                            // Update existing inventory
                            let mut dest_inventory: inventory_level_entity::ActiveModel = inv.into();
                            let new_dest_quantity =
                                dest_inventory.on_hand_quantity.clone().unwrap() + self.quantity;
                            dest_inventory.on_hand_quantity = Set(new_dest_quantity);
                            dest_inventory.updated_at = Set(Utc::now());
                            dest_inventory.update(txn).await.map_err(|e| {
                                let msg = format!("Failed to update destination inventory: {}", e);
                                error!("{}", msg);
                                ServiceError::db_error(e)
                            })?;
                        }
                        None => {
                            // Create new inventory record
                            let new_inventory = inventory_level_entity::ActiveModel {
                                id: Set(Uuid::new_v4()),
                                product_id: Set(product_id),
                                warehouse_id: Set(to_location_id.clone()),
                                product_name: Set("".to_string()), // Should be fetched from product
                                product_sku: Set("".to_string()), // Should be fetched from product
                                on_hand_quantity: Set(self.quantity),
                                reserved_quantity: Set(0),
                                allocated_quantity: Set(0),
                                available_quantity: Set(self.quantity),
                                minimum_quantity: Set(0),
                                maximum_quantity: Set(99999),
                                reorder_point: Set(10),
                                reorder_quantity: Set(100),
                                status: Set("active".to_string()),
                                last_count_date: Set(None),
                                created_at: Set(Utc::now()),
                                updated_at: Set(Utc::now()),
                            };
                            new_inventory.insert(txn).await.map_err(|e| {
                                let msg = format!("Failed to create destination inventory: {}", e);
                                error!("{}", msg);
                                ServiceError::db_error(e)
                            })?;
                        }
                    }

                    // 4. Log a record of the transfer in the inventory_transactions table
                    // (This would typically be implemented, but is omitted here for brevity)

                    Ok(())
                })
            })
            .await;

        match result {
            Ok(res) => res,
            Err(e) => {
                INVENTORY_TRANSFER_FAILURES.inc();
                return match e {
                    TransactionError::Connection(db_err) => Err(ServiceError::db_error(db_err)),
                    TransactionError::Transaction(service_err) => Err(service_err),
                };
            }
        }

        // Send event
        event_sender
            .send(Event::InventoryUpdated {
                item_id: self.product_id,
                quantity: self.quantity,
            })
            .await
            .map_err(|e| {
                let msg = format!("Failed to send event for inventory transfer: {}", e);
                error!("{}", msg);
                ServiceError::EventError(msg)
            })?;

        info!(
            product_id = %product_id,
            from_location = %from_location_id,
            to_location = %to_location_id,
            quantity = %self.quantity,
            "Inventory transferred successfully"
        );

        INVENTORY_TRANSFERS.inc();

        Ok(())
    }
}

#[cfg(all(test, feature = "mock-tests"))]
mod tests {
    use super::*;

    #[test]
    fn test_validate_transfer_command() {
        // Valid command
        let command = TransferInventoryCommand {
            product_id: Uuid::new_v4().to_string(),
            from_location_id: Uuid::new_v4().to_string(),
            to_location_id: Uuid::new_v4().to_string(),
            quantity: 10,
            lot_number: None,
            notes: None,
        };
        assert!(command.validate().is_ok());

        // Invalid - empty product_id
        let invalid_command = TransferInventoryCommand {
            product_id: "".to_string(),
            from_location_id: Uuid::new_v4().to_string(),
            to_location_id: Uuid::new_v4().to_string(),
            quantity: 10,
            lot_number: None,
            notes: None,
        };
        assert!(invalid_command.validate().is_err());

        // Invalid - zero quantity
        let invalid_command = TransferInventoryCommand {
            product_id: Uuid::new_v4().to_string(),
            from_location_id: Uuid::new_v4().to_string(),
            to_location_id: Uuid::new_v4().to_string(),
            quantity: 0,
            lot_number: None,
            notes: None,
        };
        assert!(invalid_command.validate().is_err());
    }
}
