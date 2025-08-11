use crate::{
    commands::Command,
    db::DbPool,
    errors::{ServiceError, InventoryError},
    events::{Event, EventSender},
    models::{
        inventory_level_entity::{self, Entity as InventoryLevel},
        inventory_transaction_entity::{self, Entity as InventoryTransaction, InventoryTransactionType},
    },
};
use chrono::{DateTime, Utc};
use lazy_static::lazy_static;
use prometheus::{IntCounter, IntCounterVec, Opts};
use sea_orm::{*, Set, TransactionError, TransactionTrait};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info, instrument, warn};
use uuid::Uuid;
use validator::Validate;

lazy_static! {
    static ref INVENTORY_ADJUSTMENTS: IntCounter = IntCounter::new(
        "inventory_adjustments_total",
        "Total number of inventory adjustments"
    )
    .expect("metric can be created");
    static ref INVENTORY_ADJUSTMENT_FAILURES: IntCounterVec = IntCounterVec::new(
        "inventory_adjustment_failures_total",
        "Total number of failed inventory adjustments",
        &["error_type"]
    )
    .expect("metric can be created");
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
    pub transaction_id: Uuid,
    pub product_id: Uuid,
    pub warehouse_id: Uuid,
    pub previous_quantity: i32,
    pub adjustment_quantity: i32,
    pub new_quantity: i32,
    pub adjustment_type: String,
    pub adjusted_at: DateTime<Utc>,
}

#[async_trait::async_trait]
impl Command for AdjustInventoryCommand {
    type Result = AdjustInventoryResult;
    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(
        &self,
        db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        self.validate().map_err(|e| {
            INVENTORY_ADJUSTMENT_FAILURES
                .with_label_values(&["validation_error"])
                .inc();
            let msg = format!("Invalid input: {}", e);
            error!("{}", msg);
            ServiceError::ValidationError(msg)
        })?;
        let db = db_pool.as_ref();
        // Validate reason code is valid
        self.validate_reason_code(db).await?;
        // Perform the adjustment within a transaction
        let adjusted_inventory = self.adjust_inventory_in_db(db).await?;
        // Send events and log the adjustment
        self.log_and_trigger_event(&event_sender, &adjusted_inventory)
            .await?;
        INVENTORY_ADJUSTMENTS.inc();
        Ok(adjusted_inventory)
    }
}

impl AdjustInventoryCommand {
    async fn validate_reason_code(&self, _db: &DatabaseConnection) -> Result<(), ServiceError> {
        // Here you would validate against a table of valid reason codes
        // For example: damaged, found, lost, cycle_count, etc.
        let valid_reasons = vec![
            "DAMAGED",
            "FOUND",
            "LOST",
            "CYCLE_COUNT",
            "QUALITY_ADJUSTMENT",
            "THEFT",
            "EXPIRATION",
            "SYSTEM_ADJUSTMENT",
        ];
        if !valid_reasons.contains(&self.reason_code.as_str()) {
            INVENTORY_ADJUSTMENT_FAILURES
                .with_label_values(&["invalid_reason"])
                .inc();
            return Err(ServiceError::ValidationError(format!(
                "Invalid reason code: {}. Valid codes are: {:?}",
                self.reason_code, valid_reasons
            )));
        }
        Ok(())
    }

    async fn adjust_inventory_in_db(
        &self,
        db: &DatabaseConnection,
    ) -> Result<AdjustInventoryResult, ServiceError> {
        let product_id = self.product_id;
        let warehouse_id = self.warehouse_id.clone();
        let quantity_change = self.adjustment_quantity;
        let reference_type = self.reference_number.clone();
        let reference_id = None; // No reference_id field in the command
        let reason_code = self.reason_code.clone();
        let notes = self.notes.clone();
        
        db.transaction::<_, AdjustInventoryResult, ServiceError>(|txn| {
            Box::pin(async move {
                // Get current inventory level
                let current_inventory = InventoryLevel::find()
                    .filter(
                        Condition::all()
                            .add(inventory_level_entity::Column::ProductId.eq(product_id))
                            .add(inventory_level_entity::Column::WarehouseId.eq(&warehouse_id)),
                    )
                    .one(txn)
                    .await
                    .map_err(|e| ServiceError::DatabaseError(e))?
                    .ok_or_else(|| {
                        ServiceError::NotFound(format!(
                            "Inventory level not found for product {} in warehouse {}",
                            product_id, warehouse_id
                        ))
                    })?;

                // Calculate new quantity
                let new_quantity = current_inventory.on_hand_quantity + quantity_change;
                if new_quantity < 0 {
                    return Err(ServiceError::InvalidOperation(
                        "Inventory cannot be negative".to_string(),
                    ));
                }

                // Update inventory level
                let mut inventory: inventory_level_entity::ActiveModel = current_inventory.clone().into();
                inventory.on_hand_quantity = Set(new_quantity);
                inventory.updated_at = Set(Utc::now());

                let _updated_inventory = inventory
                    .update(txn)
                    .await
                    .map_err(|e| ServiceError::DatabaseError(e))?;

                // Create inventory transaction record
                let transaction = inventory_transaction_entity::ActiveModel {
                    id: Set(Uuid::new_v4()),
                    inventory_level_id: Set(_updated_inventory.id),
                    product_id: Set(product_id),
                    warehouse_id: Set(_updated_inventory.warehouse_id),
                    transaction_type: Set(if quantity_change > 0 {
                        InventoryTransactionType::Adjustment
                    } else {
                        InventoryTransactionType::Adjustment
                    }),
                    quantity: Set(quantity_change.abs()),
                    reference_type: Set(reference_type),
                    reference_id: Set(reference_id),
                    created_at: Set(Utc::now()),
                    created_by: Set(None),
                    notes: Set(notes.or_else(|| Some(format!("Adjustment: {} - Reason: {}", quantity_change, reason_code)))),
                    ..Default::default()
                };

                let saved_transaction = transaction
                    .insert(txn)
                    .await
                    .map_err(|e| ServiceError::DatabaseError(e))?;

                Ok(AdjustInventoryResult {
                    transaction_id: saved_transaction.id,
                    product_id,
                    warehouse_id: current_inventory.warehouse_id,
                    previous_quantity: current_inventory.on_hand_quantity,
                    new_quantity,
                    adjustment_quantity: quantity_change,
                    adjustment_type: reason_code,
                    adjusted_at: Utc::now(),
                })
            })
        })
        .await
        .map_err(|e| {
            error!("Transaction failed for inventory adjustment: {}", e);
            match e {
                TransactionError::Connection(db_err) => ServiceError::DatabaseError(db_err),
                TransactionError::Transaction(service_err) => service_err,
            }
        })
    }

    async fn log_and_trigger_event(
        &self,
        event_sender: &EventSender,
        result: &AdjustInventoryResult,
    ) -> Result<(), ServiceError> {
        info!(
            warehouse_id = %self.warehouse_id,
            product_id = %self.product_id,
            adjustment = %self.adjustment_quantity,
            reason = %self.reason_code,
            new_quantity = %result.new_quantity,
            "Inventory adjusted successfully"
        );
        event_sender
            .send(Event::InventoryUpdated {
                item_id: result.product_id,
                quantity: result.new_quantity,
            })
            .await
            .map_err(|e| {
                INVENTORY_ADJUSTMENT_FAILURES
                    .with_label_values(&["event_error"])
                    .inc();
                let msg = format!("Failed to send event for inventory adjustment: {}", e);
                error!("{}", msg);
                ServiceError::EventError(msg)
            })
    }
}

// ServiceError is now centrally defined in crate::errors::ServiceError
