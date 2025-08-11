use crate::{
    commands::Command,
    db::DbPool,
    errors::{ServiceError, InventoryError},
    events::{Event, EventSender},
    models::{
        inventory_allocation_entity::{self, Entity as InventoryAllocation},
        inventory_level_entity::{self, Entity as InventoryLevel},
        inventory_reservation_entity::{self, Entity as InventoryReservation, ReservationStatus},
        AllocationStatus,
    },
};
use chrono::{DateTime, Utc};
use lazy_static::lazy_static;
use prometheus::{IntCounter, IntCounterVec};
use sea_orm::{*, Set};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info, instrument, warn};
use uuid::Uuid;
use validator::Validate;

lazy_static! {
    static ref INVENTORY_ALLOCATIONS: IntCounter = IntCounter::new(
        "inventory_allocations_total",
        "Total number of inventory allocations"
    )
    .expect("metric can be created");
    static ref INVENTORY_ALLOCATION_FAILURES: IntCounterVec = IntCounterVec::new(
        "inventory_allocation_failures_total",
        "Total number of failed inventory allocations",
        &["error_type"]
    )
    .expect("metric can be created");
    static ref INVENTORY_ALLOCATION_QUANTITY: IntCounterVec = IntCounterVec::new(
        "inventory_allocation_quantity_total",
        "Total quantity of inventory allocated",
        &["warehouse_id", "allocation_type"]
    )
    .expect("metric can be created");
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct AllocateInventoryCommand {
    pub warehouse_id: String,
    #[validate(length(min = 1))]
    pub allocations: Vec<AllocationRequest>,
    pub allocation_type: AllocationType,
    pub reference_id: Uuid,     // Order ID, Transfer ID, etc.
    pub reference_type: String, // "ORDER", "TRANSFER", etc.
    #[validate(length(max = 500))]
    pub notes: Option<String>,
    pub priority: Option<i32>,
    pub expiration: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, Deserialize, Validate, Clone)]
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

#[derive(Debug, Serialize, Deserialize, Clone)]
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
    ) -> Result<Self::Result, ServiceError> {
        self.validate().map_err(|e| {
            INVENTORY_ALLOCATION_FAILURES
                .with_label_values(&["validation_error"])
                .inc();
            let msg = format!("Invalid input: {}", e);
            error!("{}", msg);
            ServiceError::ValidationError(msg)
        })?;

        let db = db_pool.as_ref();

        // Check if there are any existing allocations for this reference
        self.check_existing_allocations(db).await?;

        // Perform the allocations within a transaction
        let allocation_results = self.allocate_inventory_in_db(db).await?;

        // Send events and log the allocations
        self.log_and_trigger_events(&event_sender, &allocation_results)
            .await?;

        INVENTORY_ALLOCATIONS.inc();
        INVENTORY_ALLOCATION_QUANTITY
            .with_label_values(&[&self.warehouse_id, &self.allocation_type.to_string()])
            .inc_by(
                allocation_results
                    .allocations
                    .iter()
                    .map(|a| a.allocated_quantity as u64)
                    .sum(),
            );

        Ok(allocation_results)
    }
}

impl AllocateInventoryCommand {
    async fn check_existing_allocations(
        &self,
        db: &DatabaseConnection,
    ) -> Result<(), ServiceError> {
        let existing_allocation = InventoryAllocation::find()
            .filter(
                inventory_allocation_entity::Column::ReferenceId
                    .eq(self.reference_id)
                    .and(
                        inventory_allocation_entity::Column::ReferenceType.eq(&self.reference_type),
                    ),
            )
            .one(db)
            .await
            .map_err(|e| ServiceError::DatabaseError(e))?;

        if existing_allocation.is_some() {
            error!(
                "Duplicate allocation found for reference ID: {}",
                self.reference_id
            );
            return Err(ServiceError::InvalidOperation(format!(
                "Duplicate allocation for reference ID: {}",
                self.reference_id
            )));
        }

        Ok(())
    }

    async fn check_reservations(
        &self,
        db: &DatabaseTransaction,
        product_id: Uuid,
        warehouse_id: Uuid,
    ) -> Result<i32, ServiceError> {
        let reserved_quantity = InventoryReservation::find()
            .filter(
                Condition::all()
                    .add(inventory_reservation_entity::Column::ProductId.eq(product_id))
                    .add(inventory_reservation_entity::Column::WarehouseId.eq(warehouse_id))
                    .add(inventory_reservation_entity::Column::Status.eq(
                        ReservationStatus::Reserved
                    ))
            )
            .all(db)
            .await
            .map_err(|e| ServiceError::DatabaseError(e))?
            .iter()
            .map(|r| r.quantity_reserved)
            .sum();

        Ok(reserved_quantity)
    }

    async fn allocate_inventory_in_db(
        &self,
        db: &DatabaseConnection,
    ) -> Result<AllocateInventoryResult, ServiceError> {
        let warehouse_id = self.warehouse_id.clone();
        let reference_id = self.reference_id;
        let reference_type = self.reference_type.clone();
        let priority = self.priority;
        let notes = self.notes.clone();
        let allocations = self.allocations.clone();
        let expiration = self.expiration;
        
        db.transaction::<_, AllocateInventoryResult, ServiceError>(|txn| {
            Box::pin(async move {
                let mut allocation_results = Vec::new();
                let mut fully_allocated = true;

                for request in &allocations {
                    // Get current inventory level
                    let inventory = InventoryLevel::find()
                        .filter(
                            Condition::all()
                                .add(
                                    inventory_level_entity::Column::WarehouseId
                                        .eq(&warehouse_id),
                                )
                                .add(
                                    inventory_level_entity::Column::ProductId
                                        .eq(request.product_id),
                                ),
                        )
                        .one(txn)
                        .await
                        .map_err(|e| ServiceError::DatabaseError(e))?
                        .ok_or_else(|| {
                            ServiceError::NotFound(format!(
                                "Inventory level not found for product {} in warehouse {}",
                                request.product_id, warehouse_id
                            ))
                        })?;

                    // Check reservations
                    let reserved_quantity: i32 = InventoryReservation::find()
                        .filter(
                            Condition::all()
                                .add(inventory_reservation_entity::Column::ProductId.eq(request.product_id))
                                .add(inventory_reservation_entity::Column::WarehouseId.eq(inventory.warehouse_id))
                                .add(inventory_reservation_entity::Column::Status.eq(
                                    ReservationStatus::Reserved
                                ))
                        )
                        .all(txn)
                        .await
                        .map_err(|e| ServiceError::DatabaseError(e))?
                        .iter()
                        .map(|r| r.quantity_reserved)
                        .sum();

                    // Calculate available quantity
                    let available_quantity = 
                        inventory.on_hand_quantity - inventory.allocated_quantity - reserved_quantity;
                    let allocation_quantity = std::cmp::min(available_quantity, request.quantity);

                    if allocation_quantity > 0 {
                        // Create allocation record
                        let allocation = inventory_allocation_entity::ActiveModel {
                            id: Set(Uuid::new_v4()),
                            inventory_level_id: Set(inventory.id),
                            product_id: Set(request.product_id),
                            warehouse_id: Set(inventory.warehouse_id),
                            reference_type: Set(reference_type.clone()),
                            reference_id: Set(reference_id),
                            quantity_allocated: Set(allocation_quantity),
                            quantity_fulfilled: Set(0),
                            status: Set(AllocationStatus::Allocated),
                            created_at: Set(Utc::now()),
                            updated_at: Set(Utc::now()),
                            expires_at: Set(expiration),
                            ..Default::default()
                        };

                        let saved_allocation = allocation
                            .insert(txn)
                            .await
                            .map_err(|e| ServiceError::DatabaseError(e))?;

                        // Update inventory allocated quantity
                        let mut inv: inventory_level_entity::ActiveModel = inventory.clone().into();
                        inv.allocated_quantity = Set(inventory.allocated_quantity + allocation_quantity);
                        inv.updated_at = Set(Utc::now());

                        inv.update(txn)
                            .await
                            .map_err(|e| ServiceError::DatabaseError(e))?;

                        allocation_results.push(AllocationResult {
                            allocation_id: saved_allocation.id,
                            warehouse_id: warehouse_id.clone(),
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
                    reference_id,
                    allocations: allocation_results,
                    fully_allocated,
                    allocation_date: Utc::now(),
                })
            })
        })
        .await
        .map_err(|e| {
            error!("Transaction failed for inventory allocation: {}", e);
            match e {
                TransactionError::Connection(db_err) => ServiceError::DatabaseError(db_err),
                TransactionError::Transaction(service_err) => service_err,
            }
        })
    }

    async fn log_and_trigger_events(
        &self,
        event_sender: &EventSender,
        results: &AllocateInventoryResult,
    ) -> Result<(), ServiceError> {
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
                item_id: results.allocations[0].product_id, // Using first allocation's product_id
                quantity: results.allocations[0].allocated_quantity,
            })
            .await
            .map_err(|e| {
                INVENTORY_ALLOCATION_FAILURES
                    .with_label_values(&["event_error"])
                    .inc();
                let msg = format!("Failed to send event for inventory allocation: {}", e);
                error!("{}", msg);
                ServiceError::EventError(msg)
            })?;

        if !results.fully_allocated {
            warn!(
                "Partial allocation for reference {}: {} items allocated out of {} requested",
                self.reference_id,
                results.allocations.len(),
                self.allocations.len()
            );
            // We could send a different event here if needed
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
        }
        .to_string()
    }
}

// ServiceError is now centrally defined in crate::errors::ServiceError
