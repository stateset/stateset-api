use crate::commands::Command;
use crate::{
    db::DbPool,
    errors::ServiceError,
    events::{Event, EventSender},
    models::{
        inventory_entity::{self, Entity as Inventory},
        inventory_level_entity::{self, Entity as InventoryLevel},
        inventory_reservation_entity::{self, Entity as InventoryReservation, ReservationStatus, ReservationType},
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
    static ref INVENTORY_RESERVATIONS: IntCounter = IntCounter::new(
        "inventory_reservations_total",
        "Total number of inventory reservations"
    )
    .expect("metric can be created");
    static ref INVENTORY_RESERVATION_FAILURES: IntCounterVec = IntCounterVec::new(
        Opts::new(
            "inventory_reservation_failures_total",
            "Total number of failed inventory reservations"
        ),
        &["error_type"]
    )
    .expect("metric can be created");
    static ref INVENTORY_RESERVATION_QUANTITY: IntCounterVec = IntCounterVec::new(
        Opts::new(
            "inventory_reservation_quantity_total",
            "Total quantity of inventory reserved"
        ),
        &["warehouse_id", "reservation_reason"]
    )
    .expect("metric can be created");
}
#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct ReserveInventoryCommand {
    pub warehouse_id: String,
    pub reference_id: Uuid,     // Order ID, Customer ID, etc.
    pub reference_type: String, // "SALES_ORDER", "CUSTOMER_HOLD", etc.
    #[validate(length(min = 1))]
    pub items: Vec<ReservationRequest>,
    pub reservation_type: ReservationType,
    #[validate(range(min = 1, max = 365))]
    pub duration_days: Option<i32>, // How long to hold the reservation
    pub priority: Option<i32>, // Higher priority reservations take precedence
    #[validate(length(max = 500))]
    pub notes: Option<String>,
    pub reservation_strategy: ReservationStrategy,
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct ReservationRequest {
    pub product_id: Uuid,
    #[validate(range(min = 1))]
    pub quantity: i32,
    pub lot_numbers: Option<Vec<String>>,
    pub location_id: Option<String>,
    pub substitutes: Option<Vec<Uuid>>, // Alternative products that can be reserved
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub enum ReservationStrategy {
    Strict,          // Must reserve exact quantity or fail
    Partial,         // Allow partial reservations
    WithSubstitutes, // Allow substitute products
    BestEffort,      // Combine partial and substitutes
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ReservationResult {
    pub reservation_id: Uuid,
    pub warehouse_id: String,
    pub product_id: Uuid,
    pub original_product_id: Uuid, // In case of substitution
    pub requested_quantity: i32,
    pub reserved_quantity: i32,
    pub lot_numbers: Option<Vec<String>>,
    pub location_id: Option<String>,
    pub expiration_date: DateTime<Utc>,
    pub status: ReservationStatus,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ReserveInventoryResult {
    pub reference_id: Uuid,
    pub reservations: Vec<ReservationResult>,
    pub fully_reserved: bool,
    pub reservation_date: DateTime<Utc>,
    pub expiration_date: DateTime<Utc>,
}
#[async_trait::async_trait]
impl Command for ReserveInventoryCommand {
    type Result = ReserveInventoryResult;
    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(
        &self,
        db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        self.validate().map_err(|e| {
            INVENTORY_RESERVATION_FAILURES
                .with_label_values(&["validation_error"])
                .inc();
            let msg = format!("Invalid input: {}", e);
            error!("{}", msg);
            ServiceError::ValidationError(msg)
        })?;
        let db = db_pool.as_ref();
        // Check if there are any existing reservations for this reference
        let existing = InventoryReservation::find()
            .filter(
                Condition::all()
                    .add(
                        inventory_reservation_entity::Column::ReferenceId
                            .eq(self.reference_id),
                    )
                    .add(
                        inventory_reservation_entity::Column::Status
                            .eq(ReservationStatus::Reserved),
                    )
                    .add(
                        inventory_reservation_entity::Column::ExpiresAt
                            .gt(Utc::now()),
                    ),
            )
            .count(db)
            .await
            .map_err(|e| ServiceError::DatabaseError(e))?;

        if existing > 0 {
            INVENTORY_RESERVATION_FAILURES
                .with_label_values(&["duplicate_reservation"])
                .inc();
            return Err(ServiceError::InvalidOperation(format!(
                "Duplicate reservation for reference ID: {}",
                self.reference_id
            )));
        }
        // Calculate expiration date
        let expiration_date = Utc::now() + chrono::Duration::days(self.duration_days.unwrap_or(7) as i64);
        // Perform the reservations within a transaction
        let reservation_results = self.reserve_inventory_in_db(db, expiration_date).await?;
        // Send events and log the reservations
        self.log_and_trigger_events(&event_sender, &reservation_results)
            .await?;
        INVENTORY_RESERVATIONS.inc();
        INVENTORY_RESERVATION_QUANTITY
            .with_label_values(&[&self.warehouse_id, &self.reservation_type.to_string()])
            .inc_by(
                reservation_results
                    .reservations
                    .iter()
                    .map(|r| r.reserved_quantity as u64)
                    .sum(),
            );
        Ok(reservation_results)
    }
}

impl ReserveInventoryCommand {
    async fn check_available_quantity(
        &self,
        db: &impl ConnectionTrait,
        product_id: Uuid,
        quantity: i32,
    ) -> Result<i32, ServiceError> {
        // Get current inventory level
        let inventory = InventoryLevel::find()
            .filter(
                Condition::all()
                    .add(inventory_level_entity::Column::WarehouseId.eq(&self.warehouse_id))
                    .add(inventory_level_entity::Column::ProductId.eq(product_id)),
            )
            .one(db)
            .await
            .map_err(|e| ServiceError::DatabaseError(e))?
            .ok_or_else(|| {
                ServiceError::NotFound(format!(
                    "Inventory level not found for product {} in warehouse {}",
                    product_id, self.warehouse_id
                ))
            })?;

        // Get existing reservations
        let warehouse_uuid = Uuid::parse_str(&self.warehouse_id).unwrap_or_else(|_| Uuid::new_v4());
        let existing_reservations = InventoryReservation::find()
            .filter(inventory_reservation_entity::Column::ProductId.eq(product_id))
            .filter(inventory_reservation_entity::Column::WarehouseId.eq(warehouse_uuid))
            .filter(inventory_reservation_entity::Column::Status.eq(ReservationStatus::Reserved))
            .all(db)
            .await
            .map_err(|e| ServiceError::DatabaseError(e))?;

        let total_reserved: i32 = existing_reservations.iter().map(|r| r.quantity_reserved).sum();

        let available_quantity = inventory.on_hand_quantity - inventory.allocated_quantity - inventory.reserved_quantity - total_reserved;

        if available_quantity < quantity {
            INVENTORY_RESERVATION_FAILURES
                .with_label_values(&["insufficient_inventory"])
                .inc();
            return Err(ServiceError::InvalidOperation(format!(
                "Insufficient inventory for product {}",
                product_id
            )));
        }

        Ok(available_quantity)
    }

    async fn reserve_inventory_in_db(
        &self,
        db: &DatabaseConnection,
        expiration_date: DateTime<Utc>,
    ) -> Result<ReserveInventoryResult, ServiceError> {
        db.transaction::<_, ReserveInventoryResult, ServiceError>(|txn| {
            Box::pin(async move {
                let mut reservation_results = Vec::new();
                let mut fully_reserved = true;
                for request in &self.items {
                    let mut reserved_quantity = 0;
                    let mut product_id = request.product_id;
                    // Try primary product
                    let available_quantity = self
                        .check_available_quantity(txn, product_id, request.quantity)
                        .await?;
                    if available_quantity > 0 {
                        let reservation_result = self
                            .create_reservation(
                                txn,
                                product_id,
                                available_quantity,
                                &request,
                            )
                            .await?;
                        reserved_quantity = reservation_result.reserved_quantity;
                    }
                    // Try substitutes if needed
                    if reserved_quantity < request.quantity
                        && matches!(
                            self.reservation_strategy,
                            ReservationStrategy::WithSubstitutes | ReservationStrategy::BestEffort
                        )
                        && request.substitutes.is_some()
                    {
                        let remaining_quantity = request.quantity - reserved_quantity;
                        for substitute_id in request.substitutes.as_ref().unwrap() {
                            let substitute_available = self
                                .check_available_quantity(txn, *substitute_id, remaining_quantity)
                                .await?;
                            if substitute_available > 0 {
                                let substitute_reservation = self
                                    .create_reservation(
                                        txn,
                                        *substitute_id,
                                        substitute_available,
                                        &request,
                                    )
                                    .await?;
                                reserved_quantity += substitute_reservation.reserved_quantity;
                                product_id = *substitute_id;
                                if reserved_quantity >= request.quantity {
                                    break;
                                }
                            }
                        }
                    }
                    if reserved_quantity < request.quantity {
                        fully_reserved = false;
                        if self.reservation_strategy == ReservationStrategy::Strict {
                            return Err(ServiceError::InvalidOperation(format!(
                                "Insufficient inventory for product {}",
                                request.product_id
                            )));
                        }
                    }
                    if reserved_quantity > 0 {
                        reservation_results.push(ReservationResult {
                            reservation_id: Uuid::new_v4(), // Set by create_reservation
                            warehouse_id: self.warehouse_id.clone(),
                            product_id,
                            original_product_id: request.product_id,
                            requested_quantity: request.quantity,
                            reserved_quantity,
                            lot_numbers: request.lot_numbers.clone(),
                            location_id: request.location_id.clone(),
                            expiration_date,
                            status: ReservationStatus::Reserved,
                        });
                    }
                }
                Ok(ReserveInventoryResult {
                    reference_id: self.reference_id,
                    reservations: reservation_results,
                    fully_reserved,
                    reservation_date: Utc::now(),
                    expiration_date,
                })
            })
        })
        .await
        .map_err(|e| {
            error!("Transaction failed for inventory reservation: {}", e);
            match e {
                TransactionError::Connection(db_err) => ServiceError::DatabaseError(db_err),
                TransactionError::Transaction(service_err) => service_err,
            }
        })
    }

    async fn create_reservation(
        &self,
        txn: &impl ConnectionTrait,
        product_id: Uuid,
        quantity: i32,
        request: &ReservationRequest,
    ) -> Result<ReservationResult, ServiceError> {
        // First find the inventory level to get the inventory_level_id
        let inventory = InventoryLevel::find()
            .filter(
                Condition::all()
                    .add(inventory_level_entity::Column::WarehouseId.eq(&self.warehouse_id))
                    .add(inventory_level_entity::Column::ProductId.eq(product_id)),
            )
            .one(txn)
            .await
            .map_err(|e| ServiceError::DatabaseError(e))?
            .ok_or_else(|| {
                ServiceError::NotFound(format!(
                    "Inventory level not found for product {} in warehouse {}",
                    product_id, self.warehouse_id
                ))
            })?;

        let warehouse_id = Uuid::parse_str(&self.warehouse_id).unwrap_or_else(|_| Uuid::new_v4());
        
        let reservation = inventory_reservation_entity::ActiveModel {
            id: Set(Uuid::new_v4()),
            inventory_level_id: Set(inventory.id),
            product_id: Set(product_id),
            warehouse_id: Set(warehouse_id),
            reservation_type: Set(self.reservation_type.clone()),
            reference_id: Set(self.reference_id),
            quantity_reserved: Set(quantity),
            quantity_released: Set(0),
            status: Set(ReservationStatus::Reserved),
            lot_numbers: Set(request
                .lot_numbers
                .as_ref()
                .map(|v| v.join(","))),
            notes: Set(self.notes.clone()),
            created_by: Set(None),
            created_at: Set(Utc::now()),
            updated_at: Set(Utc::now()),
            expires_at: Set(Some(Utc::now() + chrono::Duration::days(self.duration_days.unwrap_or(7) as i64))),
            ..Default::default()
        };
        reservation
            .insert(txn)
            .await
            .map_err(|e| ServiceError::DatabaseError(e))?;
        Ok(ReservationResult {
            reservation_id: reservation.id,
            warehouse_id: self.warehouse_id.clone(),
            product_id,
            original_product_id: request.product_id,
            requested_quantity: request.quantity,
            reserved_quantity: quantity,
            lot_numbers: request.lot_numbers.clone(),
            location_id: request.location_id.clone(),
            expiration_date: Utc::now() + chrono::Duration::days(self.duration_days.unwrap_or(7) as i64),
            status: ReservationStatus::Reserved,
        })
    }

    async fn log_and_trigger_events(
        &self,
        event_sender: &EventSender,
        results: &ReserveInventoryResult,
    ) -> Result<(), ServiceError> {
        info!(
            reference_id = %self.reference_id,
            reference_type = %self.reference_type,
            warehouse_id = %self.warehouse_id,
            reservation_count = %results.reservations.len(),
            fully_reserved = %results.fully_reserved,
            "Inventory reservation completed"
        );
        for reservation in &results.reservations {
            info!(
                product_id = %reservation.product_id,
                requested = %reservation.requested_quantity,
                reserved = %reservation.reserved_quantity,
                expiration = %reservation.expiration_date,
                "Reservation details"
            );
        }

        event_sender
            .send(Event::InventoryUpdated {
                item_id: results.reservations[0].product_id,
                quantity: -(results.reservations[0].reserved_quantity), // Negative to indicate reservation
            })
            .await
            .map_err(|e| {
                INVENTORY_RESERVATION_FAILURES
                    .with_label_values(&["event_error"])
                    .inc();
                let msg = format!("Failed to send event for inventory reservation: {}", e);
                error!("{}", msg);
                ServiceError::EventError(msg)
            })?;

        if !results.fully_reserved {
            warn!(
                "Partial reservation for reference {}: {} items reserved out of {} requested",
                self.reference_id,
                results.reservations.len(),
                self.items.len()
            );
        }

        Ok(())
    }
}
