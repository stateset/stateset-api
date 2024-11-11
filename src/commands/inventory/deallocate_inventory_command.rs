use std::sync::Arc;
use sea_orm::*;
use crate::{
    db::DbPool,
    errors::InventoryError,
    events::{Event, EventSender},
    models::{
        inventory_level_entity::{self, Entity as InventoryLevel},
        inventory_allocation_entity::{self, Entity as InventoryAllocation},
        AllocationStatus,
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
    static ref INVENTORY_DEALLOCATIONS: IntCounter = 
        IntCounter::new("inventory_deallocations_total", "Total number of inventory deallocations")
            .expect("metric can be created");

    static ref INVENTORY_DEALLOCATION_FAILURES: IntCounterVec = 
        IntCounterVec::new(
            "inventory_deallocation_failures_total",
            "Total number of failed inventory deallocations",
            &["error_type"]
        ).expect("metric can be created");

    static ref INVENTORY_DEALLOCATION_QUANTITY: IntCounterVec =
        IntCounterVec::new(
            "inventory_deallocation_quantity_total",
            "Total quantity of inventory deallocated",
            &["warehouse_id", "reason"]
        ).expect("metric can be created");
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct DeallocateInventoryCommand {
    pub reference_id: Uuid,            // Order ID, Transfer ID, etc.
    pub reference_type: String,        // "ORDER", "TRANSFER", etc.
    #[validate(length(min = 1, max = 50))]
    pub reason_code: String,
    #[validate(length(max = 500))]
    pub notes: Option<String>,
    pub deallocations: Vec<DeallocationRequest>,
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct DeallocationRequest {
    pub allocation_id: Option<Uuid>,   // Optional: Deallocate specific allocation
    pub product_id: Option<Uuid>,      // Optional: Deallocate by product
    pub quantity: Option<i32>,         // Optional: Partial deallocation
    pub lot_number: Option<String>,    // Optional: Deallocate specific lot
    pub location_id: Option<String>,   // Optional: Deallocate from specific location
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeallocationResult {
    pub allocation_id: Uuid,
    pub warehouse_id: String,
    pub product_id: Uuid,
    pub deallocated_quantity: i32,
    pub remaining_quantity: i32,
    pub status: AllocationStatus,
    pub deallocation_date: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeallocateInventoryResult {
    pub reference_id: Uuid,
    pub deallocations: Vec<DeallocationResult>,
    pub fully_deallocated: bool,
    pub deallocation_date: DateTime<Utc>,
}

#[async_trait::async_trait]
impl Command for DeallocateInventoryCommand {
    type Result = DeallocateInventoryResult;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(
        &self,
        db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, InventoryError> {
        self.validate().map_err(|e| {
            INVENTORY_DEALLOCATION_FAILURES.with_label_values(&["validation_error"]).inc();
            let msg = format!("Invalid input: {}", e);
            error!("{}", msg);
            InventoryError::ValidationError(msg)
        })?;

        let db = db_pool.as_ref();

        // Validate reason code
        self.validate_reason_code()?;

        // Perform the deallocation within a transaction
        let deallocation_results = self.deallocate_inventory_in_db(db).await?;

        // Send events and log the deallocations
        self.log_and_trigger_events(&event_sender, &deallocation_results).await?;

        INVENTORY_DEALLOCATIONS.inc();
        INVENTORY_DEALLOCATION_QUANTITY.with_label_values(&[
            &deallocation_results.deallocations[0].warehouse_id,
            &self.reason_code
        ]).inc_by(
            deallocation_results.deallocations.iter()
                .map(|d| d.deallocated_quantity as u64)
                .sum()
        );

        Ok(deallocation_results)
    }
}

impl DeallocateInventoryCommand {
    fn validate_reason_code(&self) -> Result<(), InventoryError> {
        let valid_reasons = [
            "ORDER_CANCELLED",
            "ORDER_MODIFIED",
            "TRANSFER_CANCELLED",
            "ALLOCATION_EXPIRED",
            "INVENTORY_REBALANCE",
            "SYSTEM_ADJUSTMENT",
            "MANUAL_RELEASE",
        ];

        if !valid_reasons.contains(&self.reason_code.as_str()) {
            INVENTORY_DEALLOCATION_FAILURES.with_label_values(&["invalid_reason"]).inc();
            return Err(InventoryError::InvalidReasonCode(self.reason_code.clone()));
        }

        Ok(())
    }

    async fn deallocate_inventory_in_db(
        &self,
        db: &DatabaseConnection,
    ) -> Result<DeallocateInventoryResult, InventoryError> {
        db.transaction::<_, DeallocateInventoryResult, InventoryError>(|txn| {
            Box::pin(async move {
                let mut deallocation_results = Vec::new();
                let mut fully_deallocated = true;

                // Find all relevant allocations
                let allocations = if self.deallocations.is_empty() {
                    // If no specific deallocations requested, deallocate all for the reference
                    InventoryAllocation::find()
                        .filter(
                            Condition::all()
                                .add(inventory_allocation_entity::Column::ReferenceId.eq(self.reference_id))
                                .add(inventory_allocation_entity::Column::ReferenceType.eq(&self.reference_type))
                                .add(inventory_allocation_entity::Column::Status.eq(AllocationStatus::Allocated.to_string()))
                        )
                        .all(txn)
                        .await?
                } else {
                    // Process specific deallocation requests
                    let mut allocations = Vec::new();
                    for request in &self.deallocations {
                        let mut query = InventoryAllocation::find()
                            .filter(
                                Condition::all()
                                    .add(inventory_allocation_entity::Column::ReferenceId.eq(self.reference_id))
                                    .add(inventory_allocation_entity::Column::ReferenceType.eq(&self.reference_type))
                                    .add(inventory_allocation_entity::Column::Status.eq(AllocationStatus::Allocated.to_string()))
                            );

                        if let Some(allocation_id) = request.allocation_id {
                            query = query.filter(inventory_allocation_entity::Column::Id.eq(allocation_id));
                        }
                        if let Some(product_id) = request.product_id {
                            query = query.filter(inventory_allocation_entity::Column::ProductId.eq(product_id));
                        }
                        if let Some(lot_number) = &request.lot_number {
                            query = query.filter(inventory_allocation_entity::Column::LotNumber.eq(lot_number));
                        }
                        if let Some(location_id) = &request.location_id {
                            query = query.filter(inventory_allocation_entity::Column::LocationId.eq(location_id));
                        }

                        let mut found_allocations = query.all(txn).await?;
                        allocations.append(&mut found_allocations);
                    }
                    allocations
                };

                for allocation in allocations {
                    // Get current inventory level
                    let inventory = InventoryLevel::find()
                        .filter(
                            Condition::all()
                                .add(inventory_level_entity::Column::WarehouseId.eq(&allocation.warehouse_id))
                                .add(inventory_level_entity::Column::ProductId.eq(allocation.product_id))
                        )
                        .one(txn)
                        .await?
                        .ok_or_else(|| InventoryError::NotFound(format!(
                            "Inventory level not found for product {} in warehouse {}", 
                            allocation.product_id, allocation.warehouse_id
                        )))?;

                    // Determine quantity to deallocate
                    let deallocation_quantity = if let Some(request) = self.deallocations.iter()
                        .find(|r| r.allocation_id == Some(allocation.id)) {
                        request.quantity.unwrap_or(allocation.quantity)
                    } else {
                        allocation.quantity
                    };

                    let remaining_quantity = allocation.quantity - deallocation_quantity;

                    // Update allocation status
                    let mut alloc: inventory_allocation_entity::ActiveModel = allocation.clone().into();
                    if remaining_quantity > 0 {
                        alloc.quantity = Set(remaining_quantity);
                        fully_deallocated = false;
                    } else {
                        alloc.status = Set(AllocationStatus::Deallocated.to_string());
                    }
                    alloc.last_updated_at = Set(Some(Utc::now().naive_utc()));
                    alloc.deallocation_reason = Set(Some(self.reason_code.clone()));
                    alloc.notes = Set(self.notes.clone());

                    let updated_allocation = alloc.update(txn).await?;

                    // Update inventory allocated quantity
                    let mut inv: inventory_level_entity::ActiveModel = inventory.clone().into();
                    inv.allocated_quantity = Set(inventory.allocated_quantity - deallocation_quantity);
                    inv.last_allocated_at = Set(Some(Utc::now().naive_utc()));
                    
                    inv.update(txn).await?;

                    deallocation_results.push(DeallocationResult {
                        allocation_id: updated_allocation.id,
                        warehouse_id: updated_allocation.warehouse_id,
                        product_id: updated_allocation.product_id,
                        deallocated_quantity: deallocation_quantity,
                        remaining_quantity,
                        status: if remaining_quantity > 0 { 
                            AllocationStatus::Allocated 
                        } else { 
                            AllocationStatus::Deallocated 
                        },
                        deallocation_date: updated_allocation.last_updated_at.unwrap().and_utc(),
                    });
                }

                Ok(DeallocateInventoryResult {
                    reference_id: self.reference_id,
                    deallocations: deallocation_results,
                    fully_deallocated,
                    deallocation_date: Utc::now(),
                })
            })
        }).await
    }

    async fn log_and_trigger_events(
        &self,
        event_sender: &EventSender,
        results: &DeallocateInventoryResult,
    ) -> Result<(), InventoryError> {
        info!(
            reference_id = %self.reference_id,
            reference_type = %self.reference_type,
            reason = %self.reason_code,
            deallocation_count = %results.deallocations.len(),
            fully_deallocated = %results.fully_deallocated,
            "Inventory deallocation completed"
        );

        for deallocation in &results.deallocations {
            info!(
                allocation_id = %deallocation.allocation_id,
                product_id = %deallocation.product_id,
                deallocated = %deallocation.deallocated_quantity,
                remaining = %deallocation.remaining_quantity,
                "Deallocation details"
            );
        }

        event_sender
            .send(Event::InventoryDeallocated {
                reference_id: self.reference_id,
                reference_type: self.reference_type.clone(),
                reason_code: self.reason_code.clone(),
                deallocations: results.deallocations.clone(),
                fully_deallocated: results.fully_deallocated,
            })
            .await
            .map_err(|e| {
                INVENTORY_DEALLOCATION_FAILURES.with_label_values(&["event_error"]).inc();
                let msg = format!("Failed to send event for inventory deallocation: {}", e);
                error!("{}", msg);
                InventoryError::EventError(msg)
            })
    }
}

#[derive(thiserror::Error, Debug)]
pub enum InventoryError {
    #[error("Invalid reason code: {0}")]
    InvalidReasonCode(String),
    #[error("Inventory not found: {0}")]
    NotFound(String),
    #[error("Database error: {0}")]
    DatabaseError(String),
    #[error("Event error: {0}")]
    EventError(String),
    #[error("Validation error: {0}")]
    ValidationError(String),
}