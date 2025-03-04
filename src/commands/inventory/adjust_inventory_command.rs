use std::sync::Arc;
use sea_orm::*;
use crate::{
    db::DbPool,
    errors::InventoryError,
    events::{Event, EventSender},
    models::{
        inventory_level_entity::{self, Entity as InventoryLevel},
        inventory_transaction_entity::{self, Entity as InventoryTransaction},
        InventoryTransactionType,
    },
};
use serde::{Deserialize, Serialize};
use tracing::{error, info, instrument, warn};
use uuid::Uuid;
use validator::Validate;
use prometheus::{IntCounter, IntCounterVec};
use lazy_static::lazy_static;
use chrono::{DateTime, Utc};

lazy_static! {
    static ref INVENTORY_ADJUSTMENTS: IntCounter = 
        IntCounter::new("inventory_adjustments_total", "Total number of inventory adjustments")
            .expect("metric can be created");

    static ref INVENTORY_ADJUSTMENT_FAILURES: IntCounterVec = 
        IntCounterVec::new(
            "inventory_adjustment_failures_total",
            "Total number of failed inventory adjustments",
            &["error_type"]
        ).expect("metric can be created");
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct AdjustInventoryCommand {
    pub warehouse_id: String,
    pub product_id: Uuid,
    pub adjustment_quantity: i32, // Can be positive or negative
    #[validate(length(min = 1, max = 50))]
    pub reason_code: String,
    #[validate(length(max = 500))]
    pub notes: Option<String>,
    pub lot_number: Option<String>,
    pub reference_number: Option<String>,
    pub location_id: Option<String>,
    pub version: i32, // For optimistic locking
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AdjustInventoryResult {
    pub id: Uuid,
    pub warehouse_id: String,
    pub product_id: Uuid,
    pub previous_quantity: i32,
    pub adjustment_quantity: i32,
    pub new_quantity: i32,
    pub transaction_date: DateTime<Utc>,
    pub reference_number: Option<String>,
}

#[async_trait::async_trait]
impl Command for AdjustInventoryCommand {
    type Result = AdjustInventoryResult;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(
        &self,
        db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, InventoryError> {
        self.validate().map_err(|e| {
            INVENTORY_ADJUSTMENT_FAILURES.with_label_values(&["validation_error"]).inc();
            let msg = format!("Invalid input: {}", e);
            error!("{}", msg);
            InventoryError::ValidationError(msg)
        })?;

        let db = db_pool.as_ref();

        // Validate reason code is valid
        self.validate_reason_code(db).await?;

        // Perform the adjustment within a transaction
        let adjusted_inventory = self.adjust_inventory_in_db(db).await?;

        // Send events and log the adjustment
        self.log_and_trigger_event(&event_sender, &adjusted_inventory).await?;

        INVENTORY_ADJUSTMENTS.inc();

        Ok(adjusted_inventory)
    }
}

impl AdjustInventoryCommand {
    async fn validate_reason_code(
        &self,
        db: &DatabaseConnection,
    ) -> Result<(), InventoryError> {
        // Here you would validate against a table of valid reason codes
        // For example: damaged, found, lost, cycle_count, etc.
        let valid_reasons = vec![
            "DAMAGED", "FOUND", "LOST", "CYCLE_COUNT", 
            "QUALITY_ADJUSTMENT", "THEFT", "EXPIRATION",
            "SYSTEM_ADJUSTMENT"
        ];

        if !valid_reasons.contains(&self.reason_code.to_uppercase().as_str()) {
            INVENTORY_ADJUSTMENT_FAILURES.with_label_values(&["invalid_reason"]).inc();
            return Err(InventoryError::InvalidReasonCode(self.reason_code.clone()));
        }

        Ok(())
    }

    async fn adjust_inventory_in_db(
        &self,
        db: &DatabaseConnection,
    ) -> Result<AdjustInventoryResult, InventoryError> {
        db.transaction::<_, AdjustInventoryResult, InventoryError>(|txn| {
            Box::pin(async move {
                // Get current inventory level
                let current_inventory = InventoryLevel::find()
                    .filter(
                        Condition::all()
                            .add(inventory_level_entity::Column::WarehouseId.eq(&self.warehouse_id))
                            .add(inventory_level_entity::Column::ProductId.eq(self.product_id))
                    )
                    .one(txn)
                    .await
                    .map_err(|e| InventoryError::DatabaseError(e.to_string()))?
                    .ok_or_else(|| InventoryError::NotFound(format!(
                        "Inventory level not found for product {} in warehouse {}", 
                        self.product_id, self.warehouse_id
                    )))?;

                // Check version for optimistic locking
                if current_inventory.version != self.version {
                    warn!("Concurrent modification detected for inventory {}", current_inventory.id);
                    return Err(InventoryError::ConcurrentModification(current_inventory.id));
                }

                // Validate the adjustment won't result in negative inventory
                let new_quantity = current_inventory.quantity + self.adjustment_quantity;
                if new_quantity < 0 {
                    INVENTORY_ADJUSTMENT_FAILURES.with_label_values(&["negative_inventory"]).inc();
                    return Err(InventoryError::NegativeInventory(self.product_id));
                }

                // Create inventory transaction record
                let transaction = inventory_transaction_entity::ActiveModel {
                    id: Set(Uuid::new_v4()),
                    warehouse_id: Set(self.warehouse_id.clone()),
                    product_id: Set(self.product_id),
                    transaction_type: Set(InventoryTransactionType::Adjustment.to_string()),
                    quantity: Set(self.adjustment_quantity),
                    reference_number: Set(self.reference_number.clone()),
                    reason_code: Set(self.reason_code.clone()),
                    notes: Set(self.notes.clone()),
                    lot_number: Set(self.lot_number.clone()),
                    location_id: Set(self.location_id.clone()),
                    created_at: Set(Utc::now().naive_utc()),
                    created_by: Set(None), // Could add user context if available
                    ..Default::default()
                };

                let saved_transaction = transaction.insert(txn).await
                    .map_err(|e| InventoryError::DatabaseError(e.to_string()))?;

                // Update inventory level
                let mut inventory: inventory_level_entity::ActiveModel = current_inventory.clone().into();
                inventory.quantity = Set(new_quantity);
                inventory.version = Set(self.version + 1);
                inventory.last_updated_at = Set(Utc::now().naive_utc());

                let updated_inventory = inventory.update(txn).await
                    .map_err(|e| InventoryError::DatabaseError(e.to_string()))?;

                Ok(AdjustInventoryResult {
                    id: saved_transaction.id,
                    warehouse_id: self.warehouse_id.clone(),
                    product_id: self.product_id,
                    previous_quantity: current_inventory.quantity,
                    adjustment_quantity: self.adjustment_quantity,
                    new_quantity,
                    transaction_date: saved_transaction.created_at.and_utc(),
                    reference_number: self.reference_number.clone(),
                })
            })
        }).await
    }

    async fn log_and_trigger_event(
        &self,
        event_sender: &EventSender,
        result: &AdjustInventoryResult,
    ) -> Result<(), InventoryError> {
        info!(
            warehouse_id = %self.warehouse_id,
            product_id = %self.product_id,
            adjustment = %self.adjustment_quantity,
            reason = %self.reason_code,
            new_quantity = %result.new_quantity,
            "Inventory adjusted successfully"
        );

        event_sender
            .send(Event::InventoryAdjusted {
                warehouse_id: self.warehouse_id.clone(),
                product_id: self.product_id,
                adjustment_quantity: self.adjustment_quantity,
                new_quantity: result.new_quantity,
                reason_code: self.reason_code.clone(),
                transaction_id: result.id,
                reference_number: self.reference_number.clone(),
            })
            .await
            .map_err(|e| {
                INVENTORY_ADJUSTMENT_FAILURES.with_label_values(&["event_error"]).inc();
                let msg = format!("Failed to send event for inventory adjustment: {}", e);
                error!("{}", msg);
                InventoryError::EventError(msg)
            })
    }
}

// InventoryError is now centrally defined in crate::errors::InventoryError