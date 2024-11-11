use std::sync::Arc;
use sea_orm::*;
use crate::{
    db::DbPool,
    errors::InventoryError,
    events::{Event, EventSender},
    models::{
        inventory_level_entity::{self, Entity as InventoryLevel},
        inventory_allocation_entity::{self, Entity as InventoryAllocation},
        inventory_reservation_entity::{self, Entity as InventoryReservation},
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
    static ref INVENTORY_ALLOCATIONS: IntCounter = 
        IntCounter::new("inventory_allocations_total", "Total number of inventory allocations")
            .expect("metric can be created");

    static ref INVENTORY_ALLOCATION_FAILURES: IntCounterVec = 
        IntCounterVec::new(
            "inventory_allocation_failures_total",
            "Total number of failed inventory allocations",
            &["error_type"]
        ).expect("metric can be created");

    static ref INVENTORY_ALLOCATION_QUANTITY: IntCounterVec =
        IntCounterVec::new(
            "inventory_allocation_quantity_total",
            "Total quantity of inventory allocated",
            &["warehouse_id", "allocation_type"]
        ).expect("metric can be created");
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct AllocateInventoryCommand {
    pub warehouse_id: String,
    #[validate(length(min = 1))]
    pub allocations: Vec<AllocationRequest>,
    pub allocation_type: AllocationType,
    pub reference_id: Uuid, // Order ID, Transfer ID, etc.
    pub reference_type: String, // "ORDER", "TRANSFER", etc.
    #[validate(length(max = 500))]
    pub notes: Option<String>,
    pub priority: Option<i32>,
    pub expiration: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct AllocationRequest {
    pub product_id: Uuid,
    #[validate(range(min = 1))]
    pub quantity: i32,
    pub lot_number: Option<String>,
    pub location_id: Option<String>,
    #[validate(length(max = 100))]
    pub substitution_group: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum AllocationType {
    Order,
    Transfer,
    Production,
    Reservation,
    Hold,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AllocationResult {
    pub allocation_id: Uuid,
    pub warehouse_id: String,
    pub product_id: Uuid,
    pub requested_quantity: i32,
    pub allocated_quantity: i32,
    pub status: AllocationStatus,
    pub lot_number: Option<String>,
    pub location_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AllocateInventoryResult {
    pub reference_id: Uuid,
    pub allocations: Vec<AllocationResult>,
    pub fully_allocated: bool,
    pub allocation_date: DateTime<Utc>,
}

#[async_trait::async_trait]
impl Command for AllocateInventoryCommand {
    type Result = AllocateInventoryResult;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(
        &self,
        db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, InventoryError> {
        self.validate().map_err(|e| {
            INVENTORY_ALLOCATION_FAILURES.with_label_values(&["validation_error"]).inc();
            let msg = format!("Invalid input: {}", e);
            error!("{}", msg);
            InventoryError::ValidationError(msg)
        })?;

        let db = db_pool.as_ref();

        // Check if there are any existing allocations for this reference
        self.check_existing_allocations(db).await?;

        // Perform the allocations within a transaction
        let allocation_results = self.allocate_inventory_in_db(db).await?;

        // Send events and log the allocations
        self.log_and_trigger_events(&event_sender, &allocation_results).await?;

        INVENTORY_ALLOCATIONS.inc();
        INVENTORY_ALLOCATION_QUANTITY.with_label_values(&[
            &self.warehouse_id,
            &self.allocation_type.to_string()
        ]).inc_by(
            allocation_results.allocations.iter()
                .map(|a| a.allocated_quantity as u64)
                .sum()
        );

        Ok(allocation_results)
    }
}

impl AllocateInventoryCommand {
    async fn check_existing_allocations(
        &self,
        db: &DatabaseConnection,
    ) -> Result<(), InventoryError> {
        let existing = InventoryAllocation::find()
            .filter(
                inventory_allocation_entity::Column::ReferenceId.eq(self.reference_id)
                    .and(inventory_allocation_entity::Column::ReferenceType.eq(&self.reference_type))
            )
            .count(db)
            .await
            .map_err(|e| InventoryError::DatabaseError(e.to_string()))?;

        if existing > 0 {
            INVENTORY_ALLOCATION_FAILURES.with_label_values(&["duplicate_allocation"]).inc();
            return Err(InventoryError::DuplicateAllocation(self.reference_id));
        }

        Ok(())
    }

    async fn check_reservations(
        &self,
        db: &DatabaseConnection,
        product_id: Uuid,
    ) -> Result<i32, InventoryError> {
        // Check if there are any active reservations that should be considered
        let reserved_quantity = InventoryReservation::find()
            .filter(
                Condition::all()
                    .add(inventory_reservation_entity::Column::ProductId.eq(product_id))
                    .add(inventory_reservation_entity::Column::WarehouseId.eq(&self.warehouse_id))
                    .add(inventory_reservation_entity::Column::ExpirationDate.gt(Utc::now().naive_utc()))
            )
            .sum_number(inventory_reservation_entity::Column::Quantity)
            .await
            .map_err(|e| InventoryError::DatabaseError(e.to_string()))?
            .unwrap_or(0);

        Ok(reserved_quantity)
    }

    async fn allocate_inventory_in_db(
        &self,
        db: &DatabaseConnection,
    ) -> Result<AllocateInventoryResult, InventoryError> {
        db.transaction::<_, AllocateInventoryResult, InventoryError>(|txn| {
            Box::pin(async move {
                let mut allocation_results = Vec::new();
                let mut fully_allocated = true;

                for request in &self.allocations {
                    // Get current inventory level
                    let inventory = InventoryLevel::find()
                        .filter(
                            Condition::all()
                                .add(inventory_level_entity::Column::WarehouseId.eq(&self.warehouse_id))
                                .add(inventory_level_entity::Column::ProductId.eq(request.product_id))
                        )
                        .one(txn)
                        .await
                        .map_err(|e| InventoryError::DatabaseError(e.to_string()))?
                        .ok_or_else(|| InventoryError::NotFound(format!(
                            "Inventory level not found for product {} in warehouse {}", 
                            request.product_id, self.warehouse_id
                        )))?;

                    // Check reservations
                    let reserved_quantity = self.check_reservations(txn, request.product_id).await?;

                    // Calculate available quantity
                    let available_quantity = inventory.quantity - inventory.allocated_quantity - reserved_quantity;
                    let allocation_quantity = std::cmp::min(available_quantity, request.quantity);

                    if allocation_quantity > 0 {
                        // Create allocation record
                        let allocation = inventory_allocation_entity::ActiveModel {
                            id: Set(Uuid::new_v4()),
                            warehouse_id: Set(self.warehouse_id.clone()),
                            product_id: Set(request.product_id),
                            reference_id: Set(self.reference_id),
                            reference_type: Set(self.reference_type.clone()),
                            quantity: Set(allocation_quantity),
                            status: Set(AllocationStatus::Allocated.to_string()),
                            lot_number: Set(request.lot_number.clone()),
                            location_id: Set(request.location_id.clone()),
                            notes: Set(self.notes.clone()),
                            priority: Set(self.priority),
                            expiration: Set(self.expiration.map(|d| d.naive_utc())),
                            created_at: Set(Utc::now().naive_utc()),
                            ..Default::default()
                        };

                        let saved_allocation = allocation.insert(txn).await
                            .map_err(|e| InventoryError::DatabaseError(e.to_string()))?;

                        // Update inventory allocated quantity
                        let mut inv: inventory_level_entity::ActiveModel = inventory.clone().into();
                        inv.allocated_quantity = Set(inventory.allocated_quantity + allocation_quantity);
                        inv.last_allocated_at = Set(Some(Utc::now().naive_utc()));
                        
                        inv.update(txn).await
                            .map_err(|e| InventoryError::DatabaseError(e.to_string()))?;

                        allocation_results.push(AllocationResult {
                            allocation_id: saved_allocation.id,
                            warehouse_id: self.warehouse_id.clone(),
                            product_id: request.product_id,
                            requested_quantity: request.quantity,
                            allocated_quantity: allocation_quantity,
                            status: AllocationStatus::Allocated,
                            lot_number: request.lot_number.clone(),
                            location_id: request.location_id.clone(),
                        });
                    }

                    if allocation_quantity < request.quantity {
                        fully_allocated = false;
                    }
                }

                Ok(AllocateInventoryResult {
                    reference_id: self.reference_id,
                    allocations: allocation_results,
                    fully_allocated,
                    allocation_date: Utc::now(),
                })
            })
        }).await
    }

    async fn log_and_trigger_events(
        &self,
        event_sender: &EventSender,
        results: &AllocateInventoryResult,
    ) -> Result<(), InventoryError> {
        info!(
            reference_id = %self.reference_id,
            reference_type = %self.reference_type,
            warehouse_id = %self.warehouse_id,
            allocation_count = %results.allocations.len(),
            fully_allocated = %results.fully_allocated,
            "Inventory allocation completed"
        );

        for allocation in &results.allocations {
            info!(
                allocation_id = %allocation.allocation_id,
                product_id = %allocation.product_id,
                requested = %allocation.requested_quantity,
                allocated = %allocation.allocated_quantity,
                "Allocation details"
            );
        }

        event_sender
            .send(Event::InventoryAllocated {
                reference_id: self.reference_id,
                reference_type: self.reference_type.clone(),
                warehouse_id: self.warehouse_id.clone(),
                allocations: results.allocations.clone(),
                fully_allocated: results.fully_allocated,
            })
            .await
            .map_err(|e| {
                INVENTORY_ALLOCATION_FAILURES.with_label_values(&["event_error"]).inc();
                let msg = format!("Failed to send event for inventory allocation: {}", e);
                error!("{}", msg);
                InventoryError::EventError(msg)
            })?;

        if !results.fully_allocated {
            event_sender
                .send(Event::PartialAllocationWarning {
                    reference_id: self.reference_id,
                    reference_type: self.reference_type.clone(),
                    warehouse_id: self.warehouse_id.clone(),
                })
                .await
                .map_err(|e| {
                    INVENTORY_ALLOCATION_FAILURES.with_label_values(&["event_error"]).inc();
                    let msg = format!("Failed to send partial allocation warning event: {}", e);
                    error!("{}", msg);
                    InventoryError::EventError(msg)
                })?;
        }

        Ok(())
    }
}

impl ToString for AllocationType {
    fn to_string(&self) -> String {
        match self {
            AllocationType::Order => "ORDER",
            AllocationType::Transfer => "TRANSFER",
            AllocationType::Production => "PRODUCTION",
            AllocationType::Reservation => "RESERVATION",
            AllocationType::Hold => "HOLD",
        }.to_string()
    }
}

#[derive(thiserror::Error, Debug)]
pub enum InventoryError {
    #[error("Inventory not found: {0}")]
    NotFound(String),
    #[error("Duplicate allocation for reference {0}")]
    DuplicateAllocation(Uuid),
    #[error("Insufficient inventory for product {0}")]
    InsufficientInventory(Uuid),
    #[error("Database error: {0}")]
    DatabaseError(String),
    #[error("Event error: {0}")]
    EventError(String),
    #[error("Validation error: {0}")]
    ValidationError(String),
}