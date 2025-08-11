use crate::{
    commands::Command,
    db::DbPool,
    errors::ServiceError,
    events::{Event, EventSender},
    models::{
        inventory_allocation_entity::{self, Entity as InventoryAllocation},
        inventory_level_entity::{self, Entity as InventoryLevel},
        AllocationStatus,
    },
};
use chrono::{DateTime, Utc};
use lazy_static::lazy_static;
use prometheus::{IntCounter, IntCounterVec};
use sea_orm::{*, Condition, QueryOrder, QuerySelect, Set, TransactionError, TransactionTrait};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info, instrument, warn};
use uuid::Uuid;
use validator::Validate;

lazy_static! {
    static ref INVENTORY_DEALLOCATIONS: IntCounter = IntCounter::new(
        "inventory_deallocations_total",
        "Total number of inventory deallocations"
    )
    .expect("metric can be created");
    static ref INVENTORY_DEALLOCATION_FAILURES: IntCounterVec = IntCounterVec::new(
        "inventory_deallocation_failures_total",
        "Total number of failed inventory deallocations",
        &["error_type"]
    )
    .expect("metric can be created");
    static ref INVENTORY_DEALLOCATION_QUANTITY: IntCounterVec = IntCounterVec::new(
        "inventory_deallocation_quantity_total",
        "Total quantity of inventory deallocated",
        &["warehouse_id", "reason"]
    )
    .expect("metric can be created");
}
#[derive(Debug, Serialize, Deserialize, Validate, Clone)]
pub struct DeallocateInventoryCommand {
    pub reference_id: Uuid,     // Order ID, Transfer ID, etc.
    pub reference_type: String, // "ORDER", "TRANSFER", etc.
    #[validate(length(min = 1, max = 50))]
    pub reason_code: String,
    #[validate(length(max = 500))]
    pub notes: Option<String>,
    pub deallocations: Vec<DeallocationRequest>,
}
#[derive(Debug, Serialize, Deserialize, Validate, Clone)]
pub struct DeallocationRequest {
    pub allocation_id: Option<Uuid>, // Optional: Deallocate specific allocation
    pub product_id: Option<Uuid>,    // Optional: Deallocate by product
    pub quantity: Option<i32>,       // Optional: Partial deallocation
    pub lot_number: Option<String>,  // Optional: Deallocate specific lot
    pub location_id: Option<String>, // Optional: Deallocate from specific location
}
#[derive(Debug, Serialize, Deserialize, Clone)]
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
    ) -> Result<Self::Result, ServiceError> {
        self.validate().map_err(|e| {
            INVENTORY_DEALLOCATION_FAILURES
                .with_label_values(&["validation_error"])
                .inc();
            let msg = format!("Invalid input: {}", e);
            error!("{}", msg);
            ServiceError::ValidationError(msg)
        })?;
        let db = db_pool.as_ref();
        // Validate reason code
        self.validate_reason_code()?;
        // Perform the deallocation within a transaction
        let deallocation_results = self.deallocate_inventory_in_db(db).await?;
        // Send events and log the deallocations
        self.log_and_trigger_events(&event_sender, &deallocation_results)
            .await?;
        INVENTORY_DEALLOCATIONS.inc();
        INVENTORY_DEALLOCATION_QUANTITY
            .with_label_values(&[
                &deallocation_results.deallocations[0].warehouse_id,
                &self.reason_code,
            ])
            .inc_by(
                deallocation_results
                    .deallocations
                    .iter()
                    .map(|d| d.deallocated_quantity as u64)
                    .sum(),
            );
        Ok(deallocation_results)
    }
}
impl DeallocateInventoryCommand {
    fn validate_reason_code(&self) -> Result<(), ServiceError> {
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
            INVENTORY_DEALLOCATION_FAILURES
                .with_label_values(&["invalid_reason"])
                .inc();
            return Err(ServiceError::InvalidReasonCode(self.reason_code.clone()));
        }
        Ok(())
    }

    async fn deallocate_inventory_in_db(
        &self,
        db: &DatabaseConnection,
    ) -> Result<DeallocateInventoryResult, ServiceError> {
        let self_clone = self.clone();
        db.transaction::<_, DeallocateInventoryResult, ServiceError>(move |txn| {
            Box::pin(async move { self_clone.perform_deallocations(txn).await })
        })
        .await
        .map_err(|e| {
            error!("Transaction failed for inventory deallocation: {}", e);
            match e {
                TransactionError::Connection(db_err) => ServiceError::DatabaseError(db_err),
                TransactionError::Transaction(service_err) => service_err,
            }
        })
    }

    async fn perform_deallocations(
        &self,
        txn: &DatabaseTransaction,
    ) -> Result<DeallocateInventoryResult, ServiceError> {
        let mut deallocation_results = Vec::new();
        let fully_deallocated = true;
                // Find all relevant allocations
                let allocations = if self.deallocations.is_empty() {
                    // If no specific deallocations requested, deallocate all for the reference
                    InventoryAllocation::find()
                        .filter(
                            Condition::all()
                                .add(
                                    inventory_allocation_entity::Column::ReferenceId
                                        .eq(self.reference_id),
                                )
                                .add(
                                    inventory_allocation_entity::Column::ReferenceType
                                        .eq(&self.reference_type),
                                )
                                .add(
                                    inventory_allocation_entity::Column::Status
                                        .eq(AllocationStatus::Allocated),
                                ),
                        )
                        .all(txn)
                        .await
                        .map_err(|e| ServiceError::DatabaseError(e))?
                } else {
                    // Process specific deallocation requests
                    let mut allocations = Vec::new();
                    for request in &self.deallocations {
                        let mut query = InventoryAllocation::find().filter(
                            Condition::all()
                                .add(
                                    inventory_allocation_entity::Column::ReferenceId
                                        .eq(self.reference_id),
                                )
                                .add(
                                    inventory_allocation_entity::Column::ReferenceType
                                        .eq(&self.reference_type),
                                )
                                .add(
                                    inventory_allocation_entity::Column::Status
                                        .eq(AllocationStatus::Allocated),
                                ),
                        );
                        if let Some(allocation_id) = request.allocation_id {
                            query = query
                                .filter(inventory_allocation_entity::Column::Id.eq(allocation_id));
                        }
                        if let Some(product_id) = request.product_id {
                            query = query.filter(
                                inventory_allocation_entity::Column::ProductId.eq(product_id),
                            );
                        }
                        // Removed LotNumber and LocationId filters as they don't exist in the model
                        let mut found_allocations = query.all(txn).await
                            .map_err(|e| ServiceError::DatabaseError(e))?;
                        allocations.append(&mut found_allocations);
                    }
                    allocations
                };
                for (idx, allocation) in allocations.into_iter().enumerate() {
                    // Get the corresponding deallocation request if any
                    let request = if idx < self.deallocations.len() {
                        Some(&self.deallocations[idx])
                    } else {
                        None
                    };
                    
                    // Store allocation data before move
                    let allocation_id = allocation.id;
                    let allocation_product_id = allocation.product_id;
                    let allocation_warehouse_id = allocation.warehouse_id;
                    let allocation_quantity_allocated = allocation.quantity_allocated;
                    
                    // Get current inventory level
                    let inventory = InventoryLevel::find()
                        .filter(
                            Condition::all()
                                .add(
                                    inventory_level_entity::Column::WarehouseId
                                        .eq(allocation_warehouse_id),
                                )
                                .add(
                                    inventory_level_entity::Column::ProductId
                                        .eq(allocation_product_id),
                                ),
                        )
                        .one(txn)
                        .await
                        .map_err(|e| ServiceError::DatabaseError(e))?
                        .ok_or_else(|| {
                            ServiceError::NotFound(format!(
                                "Inventory level not found for product {} in warehouse {}",
                                allocation_product_id, allocation_warehouse_id
                            ))
                        })?;
                    // Determine quantity to deallocate
                    let deallocation_quantity = if let Some(req) = request {
                        if let Some(quantity) = req.quantity {
                            std::cmp::min(quantity, allocation_quantity_allocated)
                        } else {
                            allocation_quantity_allocated
                        }
                    } else {
                        allocation_quantity_allocated
                    };
                    let remaining_quantity = allocation_quantity_allocated - deallocation_quantity;
                    // Update allocation status
                    if remaining_quantity > 0 {
                        let mut alloc: inventory_allocation_entity::ActiveModel = allocation.into();
                        alloc.quantity_allocated = Set(remaining_quantity);
                        alloc.updated_at = Set(Utc::now());
                        alloc.update(txn).await
                            .map_err(|e| ServiceError::DatabaseError(e))?;
                    } else {
                        let mut alloc: inventory_allocation_entity::ActiveModel = allocation.into();
                        alloc.status = Set(AllocationStatus::Cancelled);
                        alloc.updated_at = Set(Utc::now());
                        alloc.update(txn).await
                            .map_err(|e| ServiceError::DatabaseError(e))?;
                    }
                    // Update inventory allocated quantity
                    let mut inv: inventory_level_entity::ActiveModel = inventory.clone().into();
                    inv.allocated_quantity = Set(inventory.allocated_quantity - deallocation_quantity);
                    inv.updated_at = Set(Utc::now());
                    inv.update(txn).await
                        .map_err(|e| ServiceError::DatabaseError(e))?;
                    deallocation_results.push(DeallocationResult {
                        allocation_id,
                        warehouse_id: allocation_warehouse_id.to_string(),
                        product_id: allocation_product_id,
                        deallocated_quantity: deallocation_quantity,
                        remaining_quantity,
                        status: if remaining_quantity > 0 {
                            AllocationStatus::PartiallyAllocated
                        } else {
                            AllocationStatus::Cancelled
                        },
                        deallocation_date: Utc::now(),
                    });
                }
                Ok(DeallocateInventoryResult {
                    reference_id: self.reference_id,
                    deallocations: deallocation_results,
                    fully_deallocated,
                    deallocation_date: Utc::now(),
                })
    }

    async fn log_and_trigger_events(
        &self,
        event_sender: &EventSender,
        results: &DeallocateInventoryResult,
    ) -> Result<(), ServiceError> {
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
                item_id: results.deallocations[0].product_id, // Using first deallocation's product_id
                quantity: results.deallocations[0].deallocated_quantity,
            })
            .await
            .map_err(|e| {
                INVENTORY_DEALLOCATION_FAILURES
                    .with_label_values(&["event_error"])
                    .inc();
                let msg = format!("Failed to send event for inventory deallocation: {}", e);
                error!("{}", msg);
                ServiceError::EventError(msg)
            })?;

        Ok(())
    }
}
