use crate::commands::Command;
use crate::{
    db::DbPool,
    errors::ServiceError,
    events::{Event, EventSender},
    models::{
        inventory_level_entity::{self, Entity as InventoryLevel},
        inventory_reservation_entity::{
            self, Entity as InventoryReservation, ReservationStatus, ReservationType,
        },
    },
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use lazy_static::lazy_static;
use prometheus::{IntCounter, IntCounterVec, Opts};
use sea_orm::{*, Condition, QueryOrder, Set, TransactionError, TransactionTrait};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info, instrument};
use uuid::Uuid;
use validator::Validate;

lazy_static! {
    static ref INVENTORY_RELEASES: IntCounter = IntCounter::new(
        "inventory_releases_total",
        "Total number of inventory releases"
    )
    .expect("metric can be created");
    static ref INVENTORY_RELEASE_FAILURES: IntCounterVec = IntCounterVec::new(
        Opts::new(
            "inventory_release_failures_total",
            "Total number of failed inventory releases"
        ),
        &["error_type"]
    )
    .expect("metric can be created");
    static ref INVENTORY_RELEASE_QUANTITY: IntCounterVec = IntCounterVec::new(
        Opts::new(
            "inventory_release_quantity_total",
            "Total quantity of inventory released"
        ),
        &["warehouse_id", "release_reason"]
    )
    .expect("metric can be created");
}
#[derive(Debug, Serialize, Deserialize, Validate, Clone)]
pub struct ReleaseInventoryCommand {
    pub reference_id: Uuid,     // Order ID, Customer ID, etc.
    pub reference_type: String, // "SALES_ORDER", "CUSTOMER_HOLD", etc.
    #[validate(length(min = 1, max = 50))]
    pub reason_code: String,
    #[validate(length(max = 500))]
    pub notes: Option<String>,
    pub releases: Vec<ReleaseRequest>,
}

#[derive(Debug, Serialize, Deserialize, Validate, Clone)]
pub struct ReleaseRequest {
    pub reservation_id: Option<Uuid>, // Optional: Release specific reservation
    pub product_id: Option<Uuid>,     // Optional: Release by product
    pub quantity: Option<i32>,        // Optional: Partial release
    pub lot_numbers: Option<Vec<String>>, // Optional: Release specific lots
}
#[derive(Debug, Serialize, Deserialize, Clone)]
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
    ) -> Result<Self::Result, ServiceError> {
        self.validate().map_err(|e| {
            INVENTORY_RELEASE_FAILURES
                .with_label_values(&["validation_error"])
                .inc();
            let msg = format!("Invalid input: {}", e);
            error!("{}", msg);
            ServiceError::ValidationError(msg)
        })?;
        let db = db_pool.as_ref();
        // Validate reason code
        self.validate_reason_code()?;
        // Perform the release within a transaction
        let release_results = self.release_inventory_in_db(db).await?;
        // Send events and log the releases
        self.log_and_trigger_events(&event_sender, &release_results)
            .await?;
        INVENTORY_RELEASES.inc();
        INVENTORY_RELEASE_QUANTITY
            .with_label_values(&[&release_results.releases[0].warehouse_id, &self.reason_code])
            .inc_by(
                release_results
                    .releases
                    .iter()
                    .map(|r| r.released_quantity as u64)
                    .sum(),
            );
        Ok(release_results)
    }
}

impl ReleaseInventoryCommand {
    fn validate_reason_code(&self) -> Result<(), ServiceError> {
        let valid_reasons = [
            "ORDER_FULFILLED",
            "ORDER_CANCELLED",
            "RESERVATION_EXPIRED",
            "MANUAL_RELEASE",
            "REALLOCATION",
            "HOLD_RELEASED",
            "SYSTEM_RELEASE",
        ];
        if !valid_reasons.contains(&self.reason_code.as_str()) {
            INVENTORY_RELEASE_FAILURES
                .with_label_values(&["invalid_reason"])
                .inc();
            return Err(ServiceError::ValidationError(format!(
                "Invalid reason code: {}. Valid codes are: {:?}",
                self.reason_code, valid_reasons
            )));
        }
        Ok(())
    }

    async fn release_inventory_in_db(
        &self,
        db: &DatabaseConnection,
    ) -> Result<ReleaseInventoryResult, ServiceError> {
        let self_clone = self.clone();
        db.transaction::<_, ReleaseInventoryResult, ServiceError>(move |txn| {
            Box::pin(async move {
                let mut release_results = Vec::new();
                let mut fully_released = true;
                // Find all relevant reservations
                let reservations = if self_clone.releases.is_empty() {
                    // If no specific releases requested, release all for the reference
                    InventoryReservation::find()
                        .filter(
                            Condition::all()
                                .add(
                                    inventory_reservation_entity::Column::ReferenceId
                                        .eq(self_clone.reference_id),
                                )
                                .add(
                                    inventory_reservation_entity::Column::Status
                                        .eq(ReservationStatus::Reserved),
                                ),
                        )
                        .all(txn)
                        .await
                        .map_err(|e| ServiceError::db_error(e))?
                } else {
                    // Process specific release requests
                    let mut reservations = Vec::new();
                    for request in &self_clone.releases {
                        let mut query = InventoryReservation::find().filter(
                            Condition::all()
                                .add(
                                    inventory_reservation_entity::Column::ReferenceId
                                        .eq(self_clone.reference_id),
                                )
                                .add(
                                    inventory_reservation_entity::Column::Status
                                        .eq(ReservationStatus::Reserved),
                                ),
                        );
                        if let Some(reservation_id) = request.reservation_id {
                            query = query.filter(
                                inventory_reservation_entity::Column::Id.eq(reservation_id),
                            );
                        }
                        if let Some(product_id) = request.product_id {
                            query = query.filter(
                                inventory_reservation_entity::Column::ProductId.eq(product_id),
                            );
                        }
                        if let Some(lot_numbers) = &request.lot_numbers {
                            query = query.filter(
                                inventory_reservation_entity::Column::LotNumbers
                                    .is_in(lot_numbers.clone()),
                            );
                        }
                        let mut found_reservations = query.all(txn).await
                            .map_err(|e| ServiceError::db_error(e))?;
                        reservations.append(&mut found_reservations);
                    }
                    reservations
                };
                for (idx, reservation) in reservations.into_iter().enumerate() {
                    // Get the corresponding release request if any
                    let request = if idx < self_clone.releases.len() {
                        Some(&self_clone.releases[idx])
                    } else {
                        None
                    };
                    
                    // Determine quantity to release
                    let release_quantity = if let Some(req) = request {
                        if let Some(quantity) = req.quantity {
                            std::cmp::min(quantity, reservation.quantity_reserved)
                        } else {
                            reservation.quantity_reserved
                        }
                    } else {
                        reservation.quantity_reserved
                    };
                    let remaining_quantity = reservation.quantity_reserved - release_quantity;
                    // Update reservation status
                    let updated_reservation = if remaining_quantity > 0 {
                        let mut res: inventory_reservation_entity::ActiveModel = reservation.clone().into();
                        res.quantity_reserved = Set(remaining_quantity);
                        res.status = Set(ReservationStatus::PartiallyReserved);
                        res.updated_at = Set(Utc::now());
                        fully_released = false;
                        res.update(txn).await
                            .map_err(|e| ServiceError::db_error(e))?
                    } else {
                        let mut res: inventory_reservation_entity::ActiveModel = reservation.clone().into();
                        res.status = Set(ReservationStatus::Released);
                        res.quantity_released = Set(res.quantity_released.unwrap() + release_quantity);
                        res.updated_at = Set(Utc::now());
                        res.update(txn).await
                            .map_err(|e| ServiceError::db_error(e))?
                    };
                    
                    release_results.push(ReleaseResult {
                        reservation_id: reservation.id,
                        warehouse_id: reservation.warehouse_id.to_string(),
                        product_id: reservation.product_id,
                        released_quantity: release_quantity,
                        remaining_quantity,
                        status: if remaining_quantity > 0 {
                            ReservationStatus::PartiallyReserved
                        } else {
                            ReservationStatus::Released
                        },
                        release_date: Utc::now(),
                    });
                }
                Ok(ReleaseInventoryResult {
                    reference_id: self_clone.reference_id,
                    releases: release_results,
                    fully_released,
                    release_date: Utc::now(),
                })
            })
        })
        .await
        .map_err(|e| {
            error!("Transaction failed for inventory release: {}", e);
            match e {
                TransactionError::Connection(db_err) => ServiceError::db_error(db_err),
                TransactionError::Transaction(service_err) => service_err,
            }
        })
    }

    async fn log_and_trigger_events(
        &self,
        event_sender: &EventSender,
        results: &ReleaseInventoryResult,
    ) -> Result<(), ServiceError> {
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
            .send(Event::InventoryUpdated {
                item_id: results.releases[0].product_id,
                quantity: -(results.releases[0].released_quantity), // Negative to indicate release
            })
            .await
            .map_err(|e| {
                INVENTORY_RELEASE_FAILURES
                    .with_label_values(&["event_error"])
                    .inc();
                let msg = format!("Failed to send event for inventory release: {}", e);
                error!("{}", msg);
                ServiceError::EventError(msg)
            })?;

        Ok(())
    }
}
