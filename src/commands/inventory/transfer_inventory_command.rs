use std::sync::Arc;
use sea_orm::*;
use crate::{
    db::DbPool,
    errors::ServiceError,
    events::{Event, EventSender},
    models::inventory_items::{self, Entity as InventoryItems, Column as InventoryColumn},
};
use serde::{Deserialize, Serialize};
use tracing::{error, info, instrument};
use uuid::Uuid;
use validator::Validate;
use prometheus::{IntCounter, Counter};
use lazy_static::lazy_static;
use chrono::Utc;
use async_trait::async_trait;
use crate::commands::Command;

lazy_static! {
    static ref INVENTORY_TRANSFERS: IntCounter = 
        IntCounter::new("inventory_transfers_total", "Total number of inventory transfers")
            .expect("metric can be created");

    static ref INVENTORY_TRANSFER_FAILURES: IntCounter = 
        IntCounter::new("inventory_transfer_failures_total", "Total number of failed inventory transfers")
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
        let result = db.transaction::<_, (), ServiceError>(|txn| {
            Box::pin(async move {
                // 1. Check if there's enough inventory at the source location
                let source_inventory = InventoryItems::find()
                    .filter(InventoryColumn::ProductId.eq(product_id))
                    .filter(InventoryColumn::LocationId.eq(from_location_id))
                    .one(txn)
                    .await
                    .map_err(|e| {
                        let msg = format!("Failed to fetch source inventory: {}", e);
                        error!("{}", msg);
                        ServiceError::DatabaseError(msg)
                    })?;

                let source_inventory = match source_inventory {
                    Some(inv) => inv,
                    None => {
                        let msg = format!("No inventory found for product {} at location {}", product_id, from_location_id);
                        error!("{}", msg);
                        return Err(ServiceError::NotFoundError(msg));
                    }
                };

                // Calculate available quantity
                let available_quantity = source_inventory.quantity - source_inventory.reserved;
                if available_quantity < self.quantity {
                    let msg = format!(
                        "Insufficient inventory for transfer. Available: {}, Requested: {}",
                        available_quantity, self.quantity
                    );
                    error!("{}", msg);
                    return Err(ServiceError::BusinessRuleError(msg));
                }

                // 2. Update source inventory
                let mut source_inventory: inventory_items::ActiveModel = source_inventory.into();
                let new_source_quantity = source_inventory.quantity.clone().unwrap() - self.quantity;
                source_inventory.quantity = Set(new_source_quantity);
                source_inventory.updated_at = Set(Utc::now());
                source_inventory.update(txn).await.map_err(|e| {
                    let msg = format!("Failed to update source inventory: {}", e);
                    error!("{}", msg);
                    ServiceError::DatabaseError(msg)
                })?;

                // 3. Check if destination inventory exists
                let dest_inventory = InventoryItems::find()
                    .filter(InventoryColumn::ProductId.eq(product_id))
                    .filter(InventoryColumn::LocationId.eq(to_location_id))
                    .one(txn)
                    .await
                    .map_err(|e| {
                        let msg = format!("Failed to fetch destination inventory: {}", e);
                        error!("{}", msg);
                        ServiceError::DatabaseError(msg)
                    })?;

                match dest_inventory {
                    Some(inv) => {
                        // Update existing inventory
                        let mut dest_inventory: inventory_items::ActiveModel = inv.into();
                        let new_dest_quantity = dest_inventory.quantity.clone().unwrap() + self.quantity;
                        dest_inventory.quantity = Set(new_dest_quantity);
                        dest_inventory.updated_at = Set(Utc::now());
                        dest_inventory.update(txn).await.map_err(|e| {
                            let msg = format!("Failed to update destination inventory: {}", e);
                            error!("{}", msg);
                            ServiceError::DatabaseError(msg)
                        })?;
                    },
                    None => {
                        // Create new inventory record
                        let new_inventory = inventory_items::ActiveModel {
                            id: Set(Uuid::new_v4()),
                            product_id: Set(product_id),
                            location_id: Set(to_location_id),
                            quantity: Set(self.quantity),
                            reserved: Set(0),
                            allocated: Set(0),
                            available: Set(self.quantity),
                            created_at: Set(Utc::now()),
                            updated_at: Set(Utc::now()),
                            ..Default::default()
                        };
                        new_inventory.insert(txn).await.map_err(|e| {
                            let msg = format!("Failed to create destination inventory: {}", e);
                            error!("{}", msg);
                            ServiceError::DatabaseError(msg)
                        })?;
                    }
                }

                // 4. Log a record of the transfer in the inventory_transactions table
                // (This would typically be implemented, but is omitted here for brevity)

                Ok(())
            })
        }).await;

        if let Err(e) = &result {
            INVENTORY_TRANSFER_FAILURES.inc();
            return Err(e.clone());
        }

        // Send event for inventory transfer
        event_sender
            .send(Event::InventoryTransferred { 
                product_id, 
                from_warehouse: from_location_id, 
                to_warehouse: to_location_id, 
                quantity: self.quantity 
            })
            .await
            .map_err(|e| {
                INVENTORY_TRANSFER_FAILURES.inc();
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

#[cfg(test)]
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