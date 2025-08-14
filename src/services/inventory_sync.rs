use std::sync::Arc;
use chrono::Utc;
use rust_decimal::Decimal;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, DatabaseConnection, EntityTrait, TransactionTrait,
    QueryFilter, ColumnTrait, DbErr,
};
use tracing::{error, info, instrument, warn};

use crate::{
    entities::{
        inventory_balance::{self, Entity as InventoryBalanceEntity},
        item_master::{self, Entity as ItemMasterEntity},
        inventory_transaction::{self, Entity as InventoryTransactionEntity},
    },
    errors::ServiceError,
    events::{Event, EventSender},
};

/// Types of inventory transactions
#[derive(Debug, Clone)]
pub enum TransactionType {
    SalesOrder,
    SalesReturn,
    PurchaseReceipt,
    PurchaseReturn,
    ManufacturingConsumption,
    ManufacturingProduction,
    Adjustment,
    Transfer,
    Reservation,
    ReleaseReservation,
}

impl ToString for TransactionType {
    fn to_string(&self) -> String {
        match self {
            Self::SalesOrder => "SALES_ORDER",
            Self::SalesReturn => "SALES_RETURN",
            Self::PurchaseReceipt => "PURCHASE_RECEIPT",
            Self::PurchaseReturn => "PURCHASE_RETURN",
            Self::ManufacturingConsumption => "MFG_CONSUMPTION",
            Self::ManufacturingProduction => "MFG_PRODUCTION",
            Self::Adjustment => "ADJUSTMENT",
            Self::Transfer => "TRANSFER",
            Self::Reservation => "RESERVATION",
            Self::ReleaseReservation => "RELEASE_RESERVATION",
        }.to_string()
    }
}

/// Inventory synchronization service that maintains consistency across all inventory operations
#[derive(Clone)]
pub struct InventorySyncService {
    db: Arc<DatabaseConnection>,
    event_sender: Option<EventSender>,
}

impl InventorySyncService {
    pub fn new(db: Arc<DatabaseConnection>, event_sender: Option<EventSender>) -> Self {
        Self { db, event_sender }
    }

    /// Updates inventory balance for a specific item and location
    #[instrument(skip(self), fields(item_id, location_id, quantity))]
    pub async fn update_inventory_balance(
        &self,
        item_id: i64,
        location_id: i32,
        quantity_change: Decimal,
        transaction_type: TransactionType,
        reference_id: Option<i64>,
        reference_type: Option<String>,
    ) -> Result<inventory_balance::Model, ServiceError> {
        let db = &*self.db;
        let txn = db.begin().await.map_err(|e| {
            error!("Failed to begin transaction: {}", e);
            ServiceError::DatabaseError(e)
        })?;

        // Find or create inventory balance record
        let balance = InventoryBalanceEntity::find()
            .filter(inventory_balance::Column::InventoryItemId.eq(item_id))
            .filter(inventory_balance::Column::LocationId.eq(location_id))
            .one(&txn)
            .await
            .map_err(|e| {
                error!("Failed to fetch inventory balance: {}", e);
                ServiceError::DatabaseError(e)
            })?;

        let updated_balance = match balance {
            Some(existing) => {
                // Update existing balance
                let new_quantity = match transaction_type {
                    TransactionType::SalesOrder | 
                    TransactionType::ManufacturingConsumption |
                    TransactionType::PurchaseReturn => {
                        existing.quantity_on_hand - quantity_change.abs()
                    },
                    TransactionType::PurchaseReceipt |
                    TransactionType::ManufacturingProduction |
                    TransactionType::SalesReturn => {
                        existing.quantity_on_hand + quantity_change.abs()
                    },
                    TransactionType::Adjustment => {
                        existing.quantity_on_hand + quantity_change
                    },
                    TransactionType::Transfer => {
                        // Transfer handled separately with source and destination
                        existing.quantity_on_hand + quantity_change
                    },
                    TransactionType::Reservation => {
                        // Increase allocated, don't change on_hand
                        return self.update_allocation(
                            existing, 
                            quantity_change.abs(), 
                            true, 
                            &txn
                        ).await;
                    },
                    TransactionType::ReleaseReservation => {
                        // Decrease allocated, don't change on_hand
                        return self.update_allocation(
                            existing, 
                            quantity_change.abs(), 
                            false, 
                            &txn
                        ).await;
                    },
                };

                // Check for negative inventory
                if new_quantity < Decimal::ZERO {
                    error!("Insufficient inventory: item_id={}, location_id={}, available={}, requested={}", 
                        item_id, location_id, existing.quantity_on_hand, quantity_change);
                    return Err(ServiceError::InsufficientStock(
                        format!("Insufficient inventory for item {} at location {}", item_id, location_id)
                    ));
                }

                let mut active: inventory_balance::ActiveModel = existing.into();
                active.quantity_on_hand = Set(new_quantity);
                active.quantity_available = Set(new_quantity - active.quantity_allocated.as_ref());
                active.updated_at = Set(Utc::now().into());
                
                active.update(&txn).await.map_err(|e| {
                    error!("Failed to update inventory balance: {}", e);
                    ServiceError::DatabaseError(e)
                })?
            },
            None => {
                // Create new balance record
                if quantity_change < Decimal::ZERO {
                    return Err(ServiceError::InsufficientStock(
                        format!("Cannot create negative inventory for item {} at location {}", item_id, location_id)
                    ));
                }

                let new_balance = inventory_balance::ActiveModel {
                    inventory_balance_id: Set(0), // Will be auto-generated
                    inventory_item_id: Set(item_id),
                    location_id: Set(location_id),
                    quantity_on_hand: Set(quantity_change),
                    quantity_allocated: Set(Decimal::ZERO),
                    quantity_available: Set(quantity_change),
                    created_at: Set(Utc::now().into()),
                    updated_at: Set(Utc::now().into()),
                };

                new_balance.insert(&txn).await.map_err(|e| {
                    error!("Failed to create inventory balance: {}", e);
                    ServiceError::DatabaseError(e)
                })?
            }
        };

        // Create inventory transaction for audit trail
        self.create_inventory_transaction(
            item_id,
            location_id,
            quantity_change,
            transaction_type.to_string(),
            reference_id,
            reference_type,
            &txn,
        ).await?;

        txn.commit().await.map_err(|e| {
            error!("Failed to commit transaction: {}", e);
            ServiceError::DatabaseError(e)
        })?;

        // Send inventory update event
        if let Some(sender) = &self.event_sender {
            let _ = sender.send(Event::InventoryUpdated {
                item_id,
                location_id,
                new_quantity: updated_balance.quantity_on_hand,
                available_quantity: updated_balance.quantity_available,
            }).await;
        }

        info!("Inventory balance updated: item_id={}, location_id={}, new_quantity={}", 
            item_id, location_id, updated_balance.quantity_on_hand);

        Ok(updated_balance)
    }

    /// Updates allocation for reservations
    async fn update_allocation(
        &self,
        balance: inventory_balance::Model,
        quantity: Decimal,
        is_reservation: bool,
        txn: &sea_orm::DatabaseTransaction,
    ) -> Result<inventory_balance::Model, ServiceError> {
        let new_allocated = if is_reservation {
            balance.quantity_allocated + quantity
        } else {
            balance.quantity_allocated - quantity
        };

        if new_allocated < Decimal::ZERO {
            return Err(ServiceError::InvalidOperation(
                "Cannot release more than allocated quantity".to_string()
            ));
        }

        let new_available = balance.quantity_on_hand - new_allocated;
        if new_available < Decimal::ZERO {
            return Err(ServiceError::InsufficientStock(
                "Insufficient available inventory for reservation".to_string()
            ));
        }

        let mut active: inventory_balance::ActiveModel = balance.into();
        active.quantity_allocated = Set(new_allocated);
        active.quantity_available = Set(new_available);
        active.updated_at = Set(Utc::now().into());
        
        active.update(txn).await.map_err(|e| {
            error!("Failed to update allocation: {}", e);
            ServiceError::DatabaseError(e)
        })
    }

    /// Creates an inventory transaction record for audit trail
    async fn create_inventory_transaction(
        &self,
        item_id: i64,
        location_id: i32,
        quantity: Decimal,
        transaction_type: String,
        reference_id: Option<i64>,
        reference_type: Option<String>,
        txn: &sea_orm::DatabaseTransaction,
    ) -> Result<(), ServiceError> {
        use uuid::Uuid;
        
        // For now, create a simple transaction log entry
        // This would need to be adjusted based on your actual inventory_transaction entity structure
        let transaction = inventory_transaction::ActiveModel {
            id: Set(Uuid::new_v4()),
            product_id: Set(Uuid::new_v4()), // Would need proper item_id to product_id mapping
            location_id: Set(Uuid::new_v4()), // Would need proper location_id mapping
            r#type: Set(transaction_type),
            quantity: Set(quantity.trunc().to_string().parse::<i32>().unwrap_or(0)),
            previous_quantity: Set(0), // Would need to fetch actual previous quantity
            new_quantity: Set(0), // Would need to calculate new total
            reference_id: Set(reference_id.map(|_| Uuid::new_v4())),
            reference_type: Set(reference_type),
            reason: Set(None),
            notes: Set(None),
            created_by: Set(Uuid::new_v4()), // Would need actual user context
            created_at: Set(Utc::now()),
        };

        transaction.insert(txn).await.map_err(|e| {
            error!("Failed to create inventory transaction: {}", e);
            ServiceError::DatabaseError(e)
        })?;

        Ok(())
    }

    /// Transfers inventory between locations
    #[instrument(skip(self))]
    pub async fn transfer_inventory(
        &self,
        item_id: i64,
        from_location: i32,
        to_location: i32,
        quantity: Decimal,
        reference_id: Option<i64>,
    ) -> Result<(), ServiceError> {
        // Deduct from source location
        self.update_inventory_balance(
            item_id,
            from_location,
            -quantity,
            TransactionType::Transfer,
            reference_id,
            Some("TRANSFER_OUT".to_string()),
        ).await?;

        // Add to destination location
        self.update_inventory_balance(
            item_id,
            to_location,
            quantity,
            TransactionType::Transfer,
            reference_id,
            Some("TRANSFER_IN".to_string()),
        ).await?;

        info!("Inventory transferred: item_id={}, from={}, to={}, quantity={}", 
            item_id, from_location, to_location, quantity);

        Ok(())
    }

    /// Gets current inventory balance for an item at a location
    #[instrument(skip(self))]
    pub async fn get_inventory_balance(
        &self,
        item_id: i64,
        location_id: i32,
    ) -> Result<Option<inventory_balance::Model>, ServiceError> {
        let db = &*self.db;
        
        InventoryBalanceEntity::find()
            .filter(inventory_balance::Column::InventoryItemId.eq(item_id))
            .filter(inventory_balance::Column::LocationId.eq(location_id))
            .one(db)
            .await
            .map_err(|e| {
                error!("Failed to fetch inventory balance: {}", e);
                ServiceError::DatabaseError(e)
            })
    }

    /// Gets total inventory across all locations for an item
    #[instrument(skip(self))]
    pub async fn get_total_inventory(
        &self,
        item_id: i64,
    ) -> Result<Decimal, ServiceError> {
        let db = &*self.db;
        
        let balances = InventoryBalanceEntity::find()
            .filter(inventory_balance::Column::InventoryItemId.eq(item_id))
            .all(db)
            .await
            .map_err(|e| {
                error!("Failed to fetch inventory balances: {}", e);
                ServiceError::DatabaseError(e)
            })?;

        let total = balances.iter()
            .map(|b| b.quantity_on_hand)
            .sum();

        Ok(total)
    }

    /// Checks if sufficient inventory is available
    #[instrument(skip(self))]
    pub async fn check_availability(
        &self,
        item_id: i64,
        location_id: i32,
        required_quantity: Decimal,
    ) -> Result<bool, ServiceError> {
        let balance = self.get_inventory_balance(item_id, location_id).await?;
        
        match balance {
            Some(b) => Ok(b.quantity_available >= required_quantity),
            None => Ok(false),
        }
    }
}