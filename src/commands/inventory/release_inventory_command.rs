use std::sync::Arc;
use sea_orm::*;
use crate::{
    db::DbPool,
    errors::InventoryError,
    events::{Event, EventSender},
    models::{
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
use chrono::{DateTime, Utc};

lazy_static! {
    static ref INVENTORY_RELEASES: IntCounter = 
        IntCounter::new("inventory_releases_total", "Total number of inventory releases")
            .expect("metric can be created");

    static ref INVENTORY_RELEASE_FAILURES: IntCounterVec = 
        IntCounterVec::new(
            "inventory_release_failures_total",
            "Total number of failed inventory releases",
            &["error_type"]
        ).expect("metric can be created");

    static ref INVENTORY_RELEASE_QUANTITY: IntCounterVec =
        IntCounterVec::new(
            "inventory_release_quantity_total",
            "Total quantity of inventory released",
            &["warehouse_id", "release_reason"]
        ).expect("metric can be created");
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct ReleaseInventoryCommand {
    pub reference_id: Uuid,            // Order ID, Customer ID, etc.
    pub reference_type: String,        // "SALES_ORDER", "CUSTOMER_HOLD", etc.
    #[validate(length(min = 1, max = 50))]
    pub reason_code: String,
    #[validate(length(max = 500))]
    pub notes: Option<String>,
    pub releases: Vec<ReleaseRequest>,
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct ReleaseRequest {
    pub reservation_id: Option<Uuid>,   // Optional: Release specific reservation
    pub product_id: Option<Uuid>,       // Optional: Release by product
    pub quantity: Option<i32>,          // Optional: Partial release
    pub lot_numbers: Option<Vec<String>>, // Optional: Release specific lots
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ReleaseResult {
    pub reservation_id: Uuid,
    pub warehouse_id: String,
    pub product_id: Uuid,
    pub released_quantity: i32,
    pub remaining_quantity: i32,
    pub status: ReservationStatus,
    pub release_date: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ReleaseInventoryResult {
    pub reference_id: Uuid,
    pub releases: Vec<ReleaseResult>,
    pub fully_released: bool,
    pub release_date: DateTime<Utc>,
}

#[async_trait::async_trait]
impl Command for ReleaseInventoryCommand {
    type Result = ReleaseInventoryResult;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(
        &self,
        db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, InventoryError> {
        self.validate().map_err(|e| {
            INVENTORY_RELEASE_FAILURES.with_label_values(&["validation_error"]).inc();
            let msg = format!("Invalid input: {}", e);
            error!("{}", msg);
            InventoryError::ValidationError(msg)
        })?;

        let db = db_pool.as_ref();

        // Validate reason code
        self.validate_reason_code()?;

        // Perform the release within a transaction
        let release_results = self.release_inventory_in_db(db).await?;

        // Send events and log the releases
        self.log_and_trigger_events(&event_sender, &release_results).await?;

        INVENTORY_RELEASES.inc();
        INVENTORY_RELEASE_QUANTITY.with_label_values(&[
            &release_results.releases[0].warehouse_id,
            &self.reason_code
        ]).inc_by(
            release_results.releases.iter()
                .map(|r| r.released_quantity as u64)
                .sum()
        );

        Ok(release_results)
    }
}

impl ReleaseInventoryCommand {
    fn validate_reason_code(&self) -> Result<(), InventoryError> {
        let valid_reasons = [
            "ORDER_FULFILLED",
            "ORDER_CANCELLED",
            "RESERVATION_EXPIRED",
            "MANUAL_RELEASE",
            "REALLOCATION",
            "HOLD_RELEASED",
            "SYSTEM_RELEASE"
        ];

        if !valid_reasons.contains(&self.reason_code.as_str()) {
            INVENTORY_RELEASE_FAILURES.with_label_values(&["invalid_reason"]).inc();
            return Err(InventoryError::InvalidReasonCode(self.reason_code.clone()));
        }

        Ok(())
    }

    async fn release_inventory_in_db(
        &self,
        db: &DatabaseConnection,
    ) -> Result<ReleaseInventoryResult, InventoryError> {
        db.transaction::<_, ReleaseInventoryResult, InventoryError>(|txn| {
            Box::pin(async move {
                let mut release_results = Vec::new();
                let mut fully_released = true;

                // Find all relevant reservations
                let reservations = if self.releases.is_empty() {
                    // If no specific releases requested, release all for the reference
                    InventoryReservation::find()
                        .filter(
                            Condition::all()
                                .add(inventory_reservation_entity::Column::ReferenceId.eq(self.reference_id))
                                .add(inventory_reservation_entity::Column::ReferenceType.eq(&self.reference_type))
                                .add(inventory_reservation_entity::Column::Status.eq(ReservationStatus::Active.to_string()))
                        )
                        .all(txn)
                        .await?
                } else {
                    // Process specific release requests
                    let mut reservations = Vec::new();
                    for request in &self.releases {
                        let mut query = InventoryReservation::find()
                            .filter(
                                Condition::all()
                                    .add(inventory_reservation_entity::Column::ReferenceId.eq(self.reference_id))
                                    .add(inventory_reservation_entity::Column::ReferenceType.eq(&self.reference_type))
                                    .add(inventory_reservation_entity::Column::Status.eq(ReservationStatus::Active.to_string()))
                            );

                        if let Some(reservation_id) = request.reservation_id {
                            query = query.filter(inventory_reservation_entity::Column::Id.eq(reservation_id));
                        }
                        if let Some(product_id) = request.product_id {
                            query = query.filter(inventory_reservation_entity::Column::ProductId.eq(product_id));
                        }
                        if let Some(lot_numbers) = &request.lot_numbers {
                            query = query.filter(inventory_reservation_entity::Column::LotNumbers.is_in(lot_numbers.clone()));
                        }

                        let mut found_reservations = query.all(txn).await?;
                        reservations.append(&mut found_reservations);
                    }
                    reservations
                };

                for reservation in reservations {
                    // Determine quantity to release
                    let release_quantity = if let Some(request) = self.releases.iter()
                        .find(|r| r.reservation_id == Some(reservation.id)) {
                        request.quantity.unwrap_or(reservation.quantity)
                    } else {
                        reservation.quantity
                    };

                    let remaining_quantity = reservation.quantity - release_quantity;

                    // Update reservation status
                    let mut res: inventory_reservation_entity::ActiveModel = reservation.clone().into();
                    if remaining_quantity > 0 {
                        res.quantity = Set(remaining_quantity);
                        fully_released = false;
                    } else {
                        res.status = Set(ReservationStatus::Released.to_string());
                    }
                    res.release_date = Set(Some(Utc::now().naive_utc()));
                    res.release_reason = Set(Some(self.reason_code.clone()));
                    res.notes = Set(self.notes.clone());

                    let updated_reservation = res.update(txn).await?;

                    release_results.push(ReleaseResult {
                        reservation_id: updated_reservation.id,
                        warehouse_id: updated_reservation.warehouse_id,
                        product_id: updated_reservation.product_id,
                        released_quantity: release_quantity,
                        remaining_quantity,
                        status: if remaining_quantity > 0 { 
                            ReservationStatus::Active 
                        } else { 
                            ReservationStatus::Released 
                        },
                        release_date: updated_reservation.release_date.unwrap().and_utc(),
                    });
                }

                Ok(ReleaseInventoryResult {
                    reference_id: self.reference_id,
                    releases: release_results,
                    fully_released,
                    release_date: Utc::now(),
                })
            })
        }).await
    }

    async fn log_and_trigger_events(
        &self,
        event_sender: &EventSender,
        results: &ReleaseInventoryResult,
    ) -> Result<(), InventoryError> {
        info!(
            reference_id = %self.reference_id,
            reference_type = %self.reference_type,
            reason = %self.reason_code,
            release_count = %results.releases.len(),
            fully_released = %results.fully_released,
            "Inventory release completed"
        );

        for release in &results.releases {
            info!(
                reservation_id = %release.reservation_id,
                product_id = %release.product_id,
                released = %release.released_quantity,
                remaining = %release.remaining_quantity,
                "Release details"
            );
        }

        event_sender
            .send(Event::InventoryReleased {
                reference_id: self.reference_id,
                reference_type: self.reference_type.clone(),
                reason_code: self.reason_code.clone(),
                releases: results.releases.clone(),
                fully_released: results.fully_released,
            })
            .await
            .map_err(|e| {
                INVENTORY_RELEASE_FAILURES.with_label_values(&["event_error"]).inc();
                let msg = format!("Failed to send event for inventory release: {}", e);
                error!("{}", msg);
                InventoryError::EventError(msg)
            })
    }
}

#[derive(thiserror::Error, Debug)]
pub enum InventoryError {
    #[error("Invalid reason code: {0}")]
    InvalidReasonCode(String),
    #[error("Reservation not found: {0}")]
    NotFound(String),
    #[error("Database error: {0}")]
    DatabaseError(String),
    #[error("Event error: {0}")]
    EventError(String),
    #[error("Validation error: {0}")]
    ValidationError(String),
}