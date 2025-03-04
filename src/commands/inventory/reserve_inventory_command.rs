use std::sync::Arc;
use sea_orm::*;
use crate::{
    db::DbPool,
    errors::InventoryError,
    events::{Event, EventSender},
    models::{
        inventory_level_entity::{self, Entity as InventoryLevel},
        inventory_reservation_entity::{self, Entity as InventoryReservation},
        ReservationStatus,
    },
};
use serde::{Deserialize, Serialize};
use tracing::{error, info, instrument, warn};
use uuid::Uuid;
use validator::Validate;
use prometheus::{IntCounter, IntCounterVec};
use lazy_static::lazy_static;
use chrono::{DateTime, Duration, Utc};

lazy_static! {
    static ref INVENTORY_RESERVATIONS: IntCounter = 
        IntCounter::new("inventory_reservations_total", "Total number of inventory reservations")
            .expect("metric can be created");

    static ref INVENTORY_RESERVATION_FAILURES: IntCounterVec = 
        IntCounterVec::new(
            "inventory_reservation_failures_total",
            "Total number of failed inventory reservations",
            &["error_type"]
        ).expect("metric can be created");

    static ref INVENTORY_RESERVATION_QUANTITY: IntCounterVec =
        IntCounterVec::new(
            "inventory_reservation_quantity_total",
            "Total quantity of inventory reserved",
            &["warehouse_id", "reservation_type"]
        ).expect("metric can be created");
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct ReserveInventoryCommand {
    pub warehouse_id: String,
    pub reference_id: Uuid,         // Order ID, Customer ID, etc.
    pub reference_type: String,     // "SALES_ORDER", "CUSTOMER_HOLD", etc.
    #[validate(length(min = 1))]
    pub items: Vec<ReservationRequest>,
    pub reservation_type: ReservationType,
    #[validate(range(min = 1, max = 365))]
    pub duration_days: Option<i32>, // How long to hold the reservation
    pub priority: Option<i32>,      // Higher priority reservations take precedence
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

#[derive(Debug, Serialize, Deserialize)]
pub enum ReservationType {
    SalesOrder,
    CustomerHold,
    Production,
    QualityHold,
    PreOrder,
    SafetyStock,
}

#[derive(Debug, Serialize, Deserialize)]
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
    pub original_product_id: Uuid,  // In case of substitution
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
    ) -> Result<Self::Result, InventoryError> {
        self.validate().map_err(|e| {
            INVENTORY_RESERVATION_FAILURES.with_label_values(&["validation_error"]).inc();
            let msg = format!("Invalid input: {}", e);
            error!("{}", msg);
            InventoryError::ValidationError(msg)
        })?;

        let db = db_pool.as_ref();

        // Check for existing reservations
        self.check_existing_reservations(db).await?;

        // Calculate expiration date
        let expiration_date = Utc::now() + Duration::days(self.duration_days.unwrap_or(7) as i64);

        // Perform the reservations within a transaction
        let reservation_results = self.reserve_inventory_in_db(db, expiration_date).await?;

        // Send events and log the reservations
        self.log_and_trigger_events(&event_sender, &reservation_results).await?;

        INVENTORY_RESERVATIONS.inc();
        INVENTORY_RESERVATION_QUANTITY.with_label_values(&[
            &self.warehouse_id,
            &self.reservation_type.to_string()
        ]).inc_by(
            reservation_results.reservations.iter()
                .map(|r| r.reserved_quantity as u64)
                .sum()
        );

        Ok(reservation_results)
    }
}

impl ReserveInventoryCommand {
    async fn check_existing_reservations(
        &self,
        db: &DatabaseConnection,
    ) -> Result<(), InventoryError> {
        let existing = InventoryReservation::find()
            .filter(
                Condition::all()
                    .add(inventory_reservation_entity::Column::ReferenceId.eq(self.reference_id))
                    .add(inventory_reservation_entity::Column::ReferenceType.eq(&self.reference_type))
                    .add(inventory_reservation_entity::Column::Status.eq(ReservationStatus::Active.to_string()))
                    .add(inventory_reservation_entity::Column::ExpirationDate.gt(Utc::now().naive_utc()))
            )
            .count(db)
            .await
            .map_err(|e| InventoryError::DatabaseError(e.to_string()))?;

        if existing > 0 {
            INVENTORY_RESERVATION_FAILURES.with_label_values(&["duplicate_reservation"]).inc();
            return Err(InventoryError::DuplicateReservation(self.reference_id));
        }

        Ok(())
    }

    async fn check_available_quantity(
        &self,
        db: &DatabaseConnection,
        product_id: Uuid,
        requested_quantity: i32,
    ) -> Result<i32, InventoryError> {
        let inventory = InventoryLevel::find()
            .filter(
                Condition::all()
                    .add(inventory_level_entity::Column::WarehouseId.eq(&self.warehouse_id))
                    .add(inventory_level_entity::Column::ProductId.eq(product_id))
            )
            .one(db)
            .await
            .map_err(|e| InventoryError::DatabaseError(e.to_string()))?
            .ok_or_else(|| InventoryError::NotFound(format!(
                "Inventory level not found for product {} in warehouse {}", 
                product_id, self.warehouse_id
            )))?;

        // Get existing reservations for this product
        let existing_reservations = InventoryReservation::find()
            .filter(
                Condition::all()
                    .add(inventory_reservation_entity::Column::WarehouseId.eq(&self.warehouse_id))
                    .add(inventory_reservation_entity::Column::ProductId.eq(product_id))
                    .add(inventory_reservation_entity::Column::Status.eq(ReservationStatus::Active.to_string()))
                    .add(inventory_reservation_entity::Column::ExpirationDate.gt(Utc::now().naive_utc()))
            )
            .all(db)
            .await
            .map_err(|e| InventoryError::DatabaseError(e.to_string()))?;

        let total_reserved: i32 = existing_reservations.iter()
            .map(|r| r.quantity)
            .sum();

        let available = inventory.quantity - inventory.allocated_quantity - total_reserved;
        let reserve_quantity = std::cmp::min(available, requested_quantity);

        if reserve_quantity <= 0 {
            INVENTORY_RESERVATION_FAILURES.with_label_values(&["insufficient_inventory"]).inc();
            if self.reservation_strategy == ReservationStrategy::Strict {
                return Err(InventoryError::InsufficientInventory(product_id));
            }
        }

        Ok(reserve_quantity)
    }

    async fn reserve_inventory_in_db(
        &self,
        db: &DatabaseConnection,
        expiration_date: DateTime<Utc>,
    ) -> Result<ReserveInventoryResult, InventoryError> {
        db.transaction::<_, ReserveInventoryResult, InventoryError>(|txn| {
            Box::pin(async move {
                let mut reservation_results = Vec::new();
                let mut fully_reserved = true;

                for request in &self.items {
                    let mut reserved_quantity = 0;
                    let mut product_id = request.product_id;

                    // Try primary product
                    let available_quantity = self.check_available_quantity(txn, product_id, request.quantity).await?;
                    if available_quantity > 0 {
                        reserved_quantity = self.create_reservation(
                            txn,
                            product_id,
                            available_quantity,
                            request,
                            expiration_date,
                        ).await?;
                    }

                    // Try substitutes if needed
                    if reserved_quantity < request.quantity 
                        && matches!(self.reservation_strategy, ReservationStrategy::WithSubstitutes | ReservationStrategy::BestEffort)
                        && request.substitutes.is_some() 
                    {
                        let remaining_quantity = request.quantity - reserved_quantity;
                        for substitute_id in request.substitutes.as_ref().unwrap() {
                            let substitute_quantity = self.check_available_quantity(txn, *substitute_id, remaining_quantity).await?;
                            if substitute_quantity > 0 {
                                let additional_quantity = self.create_reservation(
                                    txn,
                                    *substitute_id,
                                    substitute_quantity,
                                    request,
                                    expiration_date,
                                ).await?;
                                reserved_quantity += additional_quantity;
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
                            return Err(InventoryError::InsufficientInventory(request.product_id));
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
                            status: ReservationStatus::Active,
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
        }).await
    }

    async fn create_reservation(
        &self,
        txn: &DatabaseConnection,
        product_id: Uuid,
        quantity: i32,
        request: &ReservationRequest,
        expiration_date: DateTime<Utc>,
    ) -> Result<i32, InventoryError> {
        let reservation = inventory_reservation_entity::ActiveModel {
            id: Set(Uuid::new_v4()),
            warehouse_id: Set(self.warehouse_id.clone()),
            product_id: Set(product_id),
            reference_id: Set(self.reference_id),
            reference_type: Set(self.reference_type.clone()),
            quantity: Set(quantity),
            status: Set("Active".to_string()),
            reservation_type: Set(self.reservation_type.clone()),
            lot_numbers: Set(request.lot_numbers.iter().collect::<Vec<_>>().join(",").into()),
            location_id: Set(request.location_id.clone()),
            priority: Set(self.priority),
            notes: Set(self.notes.clone()),
            expiration_date: Set(expiration_date.naive_utc()),
            created_at: Set(Utc::now().naive_utc()),
            created_by: Set(None), // Could add user context if available
            ..Default::default()
        };

        reservation.insert(txn)
            .await
            .map_err(|e| InventoryError::DatabaseError(e.to_string()))?;

        Ok(quantity)
    }

    async fn log_and_trigger_events(
        &self,
        event_sender: &EventSender,
        results: &ReserveInventoryResult,
    ) -> Result<(), InventoryError> {
        info!(
            reference_id = %self.reference_id,
            reference_type = %self.reference_type,
            warehouse_id = %self.warehouse_id,
            reservation_count = %results.reservations.len,
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
            .send(Event::InventoryReserved {
                reference_id: self.reference_id,
                reference_type: self.reference_type.clone(),
                warehouse_id: self.warehouse_id.clone(),
                reservations: results.reservations.clone(),
                fully_reserved: results.fully_reserved,
                expiration_date: results.expiration_date,
            })
            .await
            .map_err(|e| {
                INVENTORY_RESERVATION_FAILURES.with_label_values(&["event_error"]).inc();
                let msg = format!("Failed to send event for inventory reservation: {}", e);
                error!("{}", msg);
                InventoryError::EventError(msg)
            })?;

        if !results.fully_reserved {
            event_sender
                .send(Event::PartialReservationWarning {
                    reference_id: self.reference_id,
                    reference_type: self.reference_type.clone(),
                    warehouse_id: self.warehouse_id.clone(),
                    reservations: results.reservations.clone(),
                    expiration_date: results.expiration_date,
                })
                .await
                .map_err(|e| InventoryError::EventError(e.to_string()))?;
        }

        Ok(())
    }
}