use chrono::{NaiveDate, Utc};
use metrics::{counter, histogram};
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter,
    TransactionTrait,
};
use std::sync::Arc;
use tracing::{error, info, instrument, warn};

use crate::{
    entities::{
        bom_header::{self, Entity as BomHeaderEntity},
        item_master::Entity as ItemMasterEntity,
        manufacturing_work_orders::{self, Entity as WorkOrderEntity},
    },
    errors::ServiceError,
    events::{Event, EventSender},
    services::{
        bom::BomService,
        inventory_sync::{InventorySyncService, TransactionType},
    },
};

/// Manufacturing service for managing work orders and production
#[derive(Clone)]
pub struct ManufacturingService {
    db: Arc<DatabaseConnection>,
    inventory_sync: Arc<InventorySyncService>,
    bom_service: Arc<BomService>,
    event_sender: Option<EventSender>,
}

impl ManufacturingService {
    pub fn new(
        db: Arc<DatabaseConnection>,
        inventory_sync: Arc<InventorySyncService>,
        bom_service: Arc<BomService>,
        event_sender: Option<EventSender>,
    ) -> Self {
        Self {
            db,
            inventory_sync,
            bom_service,
            event_sender,
        }
    }

    /// Creates a new manufacturing work order
    #[instrument(skip(self))]
    pub async fn create_work_order(
        &self,
        work_order_number: String,
        item_id: i64,
        organization_id: i64,
        quantity_to_build: Decimal,
        scheduled_start_date: NaiveDate,
        scheduled_completion_date: NaiveDate,
        location_id: i32,
    ) -> Result<manufacturing_work_orders::Model, ServiceError> {
        // Input validation
        if work_order_number.trim().is_empty() {
            return Err(ServiceError::InvalidInput(
                "Work order number cannot be empty".to_string(),
            ));
        }

        if quantity_to_build <= Decimal::ZERO {
            return Err(ServiceError::InvalidInput(format!(
                "Quantity to build must be positive, got: {}",
                quantity_to_build
            )));
        }

        if scheduled_completion_date < scheduled_start_date {
            return Err(ServiceError::InvalidInput(format!(
                "Scheduled completion date ({}) cannot be before scheduled start date ({})",
                scheduled_completion_date, scheduled_start_date
            )));
        }

        if item_id <= 0 {
            return Err(ServiceError::InvalidInput(format!(
                "Item ID must be positive, got: {}",
                item_id
            )));
        }

        if organization_id <= 0 {
            return Err(ServiceError::InvalidInput(format!(
                "Organization ID must be positive, got: {}",
                organization_id
            )));
        }

        if location_id <= 0 {
            return Err(ServiceError::InvalidInput(format!(
                "Location ID must be positive, got: {}",
                location_id
            )));
        }

        let db = &*self.db;
        let txn = db.begin().await.map_err(|e| ServiceError::db_error(e))?;

        // Verify item exists and has a BOM
        let item = ItemMasterEntity::find_by_id(item_id)
            .one(&txn)
            .await
            .map_err(|e| ServiceError::db_error(e))?
            .ok_or_else(|| ServiceError::NotFound(format!("Item {} not found", item_id)))?;

        // Find active BOM for the item
        let bom = BomHeaderEntity::find()
            .filter(bom_header::Column::ItemId.eq(item_id))
            .filter(bom_header::Column::StatusCode.eq("ACTIVE"))
            .one(&txn)
            .await
            .map_err(|e| ServiceError::db_error(e))?
            .ok_or_else(|| {
                ServiceError::NotFound(format!("No active BOM found for item {}", item_id))
            })?;

        // Validate component availability and reserve if possible
        let availability = self
            .bom_service
            .validate_component_availability(bom.bom_id, quantity_to_build, location_id)
            .await?;

        if !availability.can_produce {
            warn!(
                "Insufficient components for work order: {:?}",
                availability.shortages
            );
            // We'll create the work order but mark it as pending materials
        }

        let work_order = manufacturing_work_orders::ActiveModel {
            work_order_id: Set(0), // Auto-generated
            work_order_number: Set(work_order_number.clone()),
            item_id: Set(Some(item_id)),
            organization_id: Set(organization_id),
            scheduled_start_date: Set(Some(scheduled_start_date)),
            scheduled_completion_date: Set(Some(scheduled_completion_date)),
            actual_start_date: Set(None),
            actual_completion_date: Set(None),
            status_code: Set(Some(if availability.can_produce {
                "READY".to_string()
            } else {
                "PENDING_MATERIALS".to_string()
            })),
            quantity_to_build: Set(Some(quantity_to_build)),
            quantity_completed: Set(Some(Decimal::ZERO)),
            created_at: Set(Utc::now().into()),
            updated_at: Set(Utc::now().into()),
        };

        let created = work_order.insert(&txn).await.map_err(|e| {
            error!("Failed to create work order: {}", e);
            ServiceError::db_error(e)
        })?;

        // Reserve components if available
        if availability.can_produce {
            match self
                .bom_service
                .reserve_components_for_work_order(
                    bom.bom_id,
                    quantity_to_build,
                    location_id,
                    created.work_order_id,
                )
                .await
            {
                Ok(reservations) => {
                    info!(
                        "Reserved {} components for work order {}",
                        reservations.len(),
                        created.work_order_id
                    );

                    // Send materials reserved event
                    if let Some(sender) = &self.event_sender {
                        sender
                            .send_or_log(Event::WorkOrderMaterialsReserved {
                                work_order_id: created.work_order_id,
                                item_count: reservations.len(),
                            })
                            .await;
                    }
                }
                Err(e) => {
                    error!("Failed to reserve components: {}", e);
                    // Rollback work order creation
                    return Err(e);
                }
            }
        } else {
            // Send component shortage events for each shortage
            for shortage in &availability.shortages {
                if let Some(sender) = &self.event_sender {
                    sender
                        .send_or_log(Event::ComponentShortageDetected {
                            work_order_id: created.work_order_id,
                            item_id: shortage.item_id,
                            required_quantity: shortage.required,
                            available_quantity: shortage.available,
                            shortage_quantity: shortage.shortage,
                        })
                        .await;
                }
            }
        }

        txn.commit().await.map_err(|e| ServiceError::db_error(e))?;

        // Record metrics
        counter!("manufacturing.work_orders.created", 1);
        if availability.can_produce {
            counter!("manufacturing.work_orders.ready", 1);
        } else {
            counter!("manufacturing.work_orders.pending_materials", 1);
        }
        histogram!(
            "manufacturing.work_orders.quantity",
            quantity_to_build.to_f64().unwrap_or(0.0)
        );

        // Send creation and schedule events
        if let Some(sender) = &self.event_sender {
            sender
                .send_or_log(Event::WorkOrderCreated {
                    work_order_id: created.work_order_id,
                    item_id,
                    quantity: quantity_to_build,
                })
                .await;

            sender
                .send_or_log(Event::WorkOrderScheduled {
                    work_order_id: created.work_order_id,
                    scheduled_start: scheduled_start_date,
                    scheduled_completion: scheduled_completion_date,
                })
                .await;
        }

        info!(
            "Work order created: {} for item {} quantity {}",
            work_order_number, item_id, quantity_to_build
        );

        Ok(created)
    }

    /// Starts a work order and consumes components
    #[instrument(skip(self))]
    pub async fn start_work_order(
        &self,
        work_order_id: i64,
        location_id: i32,
    ) -> Result<manufacturing_work_orders::Model, ServiceError> {
        let db = &*self.db;
        let txn = db.begin().await.map_err(|e| ServiceError::db_error(e))?;

        // Get work order
        let work_order = WorkOrderEntity::find_by_id(work_order_id)
            .one(&txn)
            .await
            .map_err(|e| ServiceError::db_error(e))?
            .ok_or_else(|| {
                ServiceError::NotFound(format!("Work order {} not found", work_order_id))
            })?;

        // Validate status
        if work_order.status_code != Some("READY".to_string()) {
            return Err(ServiceError::InvalidOperation(format!(
                "Work order {} is not ready to start. Current status: {:?}",
                work_order_id, work_order.status_code
            )));
        }

        let item_id = work_order
            .item_id
            .ok_or_else(|| ServiceError::InvalidOperation("Work order has no item".to_string()))?;
        let quantity = work_order.quantity_to_build.ok_or_else(|| {
            ServiceError::InvalidOperation("Work order has no quantity".to_string())
        })?;

        // Find BOM
        let bom = BomHeaderEntity::find()
            .filter(bom_header::Column::ItemId.eq(item_id))
            .filter(bom_header::Column::StatusCode.eq("ACTIVE"))
            .one(&txn)
            .await
            .map_err(|e| ServiceError::db_error(e))?
            .ok_or_else(|| {
                ServiceError::NotFound(format!("No active BOM found for item {}", item_id))
            })?;

        // Consume reserved components (releases reservation and consumes inventory)
        self.bom_service
            .consume_reserved_components(bom.bom_id, quantity, location_id, work_order_id)
            .await?;

        // Update work order status
        let mut active: manufacturing_work_orders::ActiveModel = work_order.into();
        active.status_code = Set(Some("IN_PROGRESS".to_string()));
        active.actual_start_date = Set(Some(Utc::now().date_naive()));
        active.updated_at = Set(Utc::now().into());

        let updated = active.update(&txn).await.map_err(|e| {
            error!("Failed to update work order: {}", e);
            ServiceError::db_error(e)
        })?;

        txn.commit().await.map_err(|e| ServiceError::db_error(e))?;

        // Record metrics
        counter!("manufacturing.work_orders.started", 1);
        histogram!(
            "manufacturing.components.consumed",
            quantity.to_f64().unwrap_or(0.0)
        );

        // Send event
        if let Some(sender) = &self.event_sender {
            sender
                .send_or_log(Event::WorkOrderStarted {
                    work_order_id,
                    item_id,
                })
                .await;
        }

        info!("Work order {} started, components consumed", work_order_id);

        Ok(updated)
    }

    /// Completes a work order and adds finished goods to inventory
    #[instrument(skip(self))]
    pub async fn complete_work_order(
        &self,
        work_order_id: i64,
        completed_quantity: Decimal,
        location_id: i32,
    ) -> Result<manufacturing_work_orders::Model, ServiceError> {
        // Input validation
        if work_order_id <= 0 {
            return Err(ServiceError::InvalidInput(format!(
                "Work order ID must be positive, got: {}",
                work_order_id
            )));
        }

        if completed_quantity <= Decimal::ZERO {
            return Err(ServiceError::InvalidInput(format!(
                "Completed quantity must be positive, got: {}",
                completed_quantity
            )));
        }

        if location_id <= 0 {
            return Err(ServiceError::InvalidInput(format!(
                "Location ID must be positive, got: {}",
                location_id
            )));
        }

        let db = &*self.db;
        let txn = db.begin().await.map_err(|e| ServiceError::db_error(e))?;

        // Get work order
        let work_order = WorkOrderEntity::find_by_id(work_order_id)
            .one(&txn)
            .await
            .map_err(|e| ServiceError::db_error(e))?
            .ok_or_else(|| {
                ServiceError::NotFound(format!("Work order {} not found", work_order_id))
            })?;

        // Validate status
        if work_order.status_code != Some("IN_PROGRESS".to_string()) {
            return Err(ServiceError::InvalidOperation(format!(
                "Work order {} is not in progress. Current status: {:?}",
                work_order_id, work_order.status_code
            )));
        }

        let item_id = work_order
            .item_id
            .ok_or_else(|| ServiceError::InvalidOperation("Work order has no item".to_string()))?;

        // Add finished goods to inventory
        self.inventory_sync
            .update_inventory_balance(
                item_id,
                location_id,
                completed_quantity,
                TransactionType::ManufacturingProduction,
                Some(work_order_id),
                Some("WORK_ORDER".to_string()),
            )
            .await?;

        // Update work order
        let current_completed = work_order.quantity_completed.unwrap_or(Decimal::ZERO);
        let total_completed = current_completed + completed_quantity;
        let quantity_to_build = work_order.quantity_to_build.unwrap_or(Decimal::ZERO);

        let mut active: manufacturing_work_orders::ActiveModel = work_order.into();
        active.quantity_completed = Set(Some(total_completed));

        if total_completed >= quantity_to_build {
            active.status_code = Set(Some("COMPLETED".to_string()));
            active.actual_completion_date = Set(Some(Utc::now().date_naive()));
        } else {
            active.status_code = Set(Some("PARTIALLY_COMPLETED".to_string()));
        }

        active.updated_at = Set(Utc::now().into());

        let updated = active.update(&txn).await.map_err(|e| {
            error!("Failed to update work order: {}", e);
            ServiceError::db_error(e)
        })?;

        txn.commit().await.map_err(|e| ServiceError::db_error(e))?;

        // Record metrics
        if updated.status_code == Some("COMPLETED".to_string()) {
            counter!("manufacturing.work_orders.completed", 1);

            // Calculate cycle time if we have both start and completion dates
            if let (Some(start), Some(end)) = (
                updated.actual_start_date,
                updated.actual_completion_date,
            ) {
                let cycle_time_days = (end - start).num_days();
                histogram!("manufacturing.work_orders.cycle_time_days", cycle_time_days as f64);
            }
        } else {
            counter!("manufacturing.work_orders.partially_completed", 1);
        }
        histogram!(
            "manufacturing.finished_goods.produced",
            completed_quantity.to_f64().unwrap_or(0.0)
        );

        // Calculate yield percentage
        if let Some(planned_qty) = updated.quantity_to_build {
            let yield_percentage = (total_completed / planned_qty * Decimal::from(100))
                .to_f64()
                .unwrap_or(0.0);
            histogram!("manufacturing.work_orders.yield_percentage", yield_percentage);
        }

        // Send event
        if let Some(sender) = &self.event_sender {
            sender
                .send_or_log(Event::WorkOrderCompleted {
                    work_order_id,
                    item_id,
                    quantity_completed: completed_quantity,
                })
                .await;
        }

        info!(
            "Work order {} completed with quantity {}",
            work_order_id, completed_quantity
        );

        Ok(updated)
    }

    /// Gets work order status
    #[instrument(skip(self))]
    pub async fn get_work_order_status(
        &self,
        work_order_id: i64,
    ) -> Result<WorkOrderStatus, ServiceError> {
        let db = &*self.db;

        let work_order = WorkOrderEntity::find_by_id(work_order_id)
            .one(db)
            .await
            .map_err(|e| ServiceError::db_error(e))?
            .ok_or_else(|| {
                ServiceError::NotFound(format!("Work order {} not found", work_order_id))
            })?;

        Ok(WorkOrderStatus {
            work_order_id,
            status: work_order
                .status_code
                .unwrap_or_else(|| "UNKNOWN".to_string()),
            quantity_to_build: work_order.quantity_to_build.unwrap_or(Decimal::ZERO),
            quantity_completed: work_order.quantity_completed.unwrap_or(Decimal::ZERO),
            actual_start_date: work_order.actual_start_date,
            actual_completion_date: work_order.actual_completion_date,
        })
    }

    /// Cancels a work order (if not started) and releases component reservations
    #[instrument(skip(self))]
    pub async fn cancel_work_order(
        &self,
        work_order_id: i64,
        location_id: i32,
    ) -> Result<(), ServiceError> {
        let db = &*self.db;

        let work_order = WorkOrderEntity::find_by_id(work_order_id)
            .one(db)
            .await
            .map_err(|e| ServiceError::db_error(e))?
            .ok_or_else(|| {
                ServiceError::NotFound(format!("Work order {} not found", work_order_id))
            })?;

        // Can only cancel if not started
        if work_order.actual_start_date.is_some() {
            return Err(ServiceError::InvalidOperation(
                "Cannot cancel work order that has already started".to_string(),
            ));
        }

        let item_id = work_order
            .item_id
            .ok_or_else(|| ServiceError::InvalidOperation("Work order has no item".to_string()))?;
        let quantity = work_order.quantity_to_build.ok_or_else(|| {
            ServiceError::InvalidOperation("Work order has no quantity".to_string())
        })?;

        // Find BOM to release reservations
        let bom = BomHeaderEntity::find()
            .filter(bom_header::Column::ItemId.eq(item_id))
            .filter(bom_header::Column::StatusCode.eq("ACTIVE"))
            .one(db)
            .await
            .map_err(|e| ServiceError::db_error(e))?
            .ok_or_else(|| {
                ServiceError::NotFound(format!("No active BOM found for item {}", item_id))
            })?;

        // Release component reservations if work order was in READY status
        if work_order.status_code == Some("READY".to_string()) {
            self.bom_service
                .release_component_reservations(bom.bom_id, quantity, location_id, work_order_id)
                .await?;

            info!(
                "Released component reservations for cancelled work order {}",
                work_order_id
            );

            // Send materials released event
            if let Some(sender) = &self.event_sender {
                sender
                    .send_or_log(Event::WorkOrderMaterialsReleased {
                        work_order_id,
                        reason: "Work order cancelled".to_string(),
                    })
                    .await;
            }
        }

        let mut active: manufacturing_work_orders::ActiveModel = work_order.into();
        active.status_code = Set(Some("CANCELLED".to_string()));
        active.updated_at = Set(Utc::now().into());

        active.update(db).await.map_err(|e| {
            error!("Failed to cancel work order: {}", e);
            ServiceError::db_error(e)
        })?;

        // Record metrics
        counter!("manufacturing.work_orders.cancelled", 1);

        info!("Work order {} cancelled", work_order_id);

        Ok(())
    }

    /// Puts a work order on hold
    #[instrument(skip(self))]
    pub async fn hold_work_order(
        &self,
        work_order_id: i64,
        reason: Option<String>,
    ) -> Result<manufacturing_work_orders::Model, ServiceError> {
        let db = &*self.db;

        let work_order = WorkOrderEntity::find_by_id(work_order_id)
            .one(db)
            .await
            .map_err(|e| ServiceError::db_error(e))?
            .ok_or_else(|| {
                ServiceError::NotFound(format!("Work order {} not found", work_order_id))
            })?;

        // Can only hold work orders that are READY or IN_PROGRESS
        let current_status = work_order
            .status_code
            .as_ref()
            .ok_or_else(|| ServiceError::InvalidOperation("Work order has no status".to_string()))?;

        if current_status != "READY" && current_status != "IN_PROGRESS" {
            return Err(ServiceError::InvalidOperation(format!(
                "Cannot hold work order with status: {}",
                current_status
            )));
        }

        let mut active: manufacturing_work_orders::ActiveModel = work_order.into();
        active.status_code = Set(Some("ON_HOLD".to_string()));
        active.updated_at = Set(Utc::now().into());

        let updated = active.update(db).await.map_err(|e| {
            error!("Failed to hold work order: {}", e);
            ServiceError::db_error(e)
        })?;

        // Record metrics
        counter!("manufacturing.work_orders.on_hold", 1);

        // Send event
        if let Some(sender) = &self.event_sender {
            sender
                .send_or_log(Event::WorkOrderOnHold {
                    work_order_id,
                    reason,
                })
                .await;
        }

        info!("Work order {} put on hold", work_order_id);

        Ok(updated)
    }

    /// Resumes a work order from hold
    #[instrument(skip(self))]
    pub async fn resume_work_order(
        &self,
        work_order_id: i64,
    ) -> Result<manufacturing_work_orders::Model, ServiceError> {
        let db = &*self.db;

        let work_order = WorkOrderEntity::find_by_id(work_order_id)
            .one(db)
            .await
            .map_err(|e| ServiceError::db_error(e))?
            .ok_or_else(|| {
                ServiceError::NotFound(format!("Work order {} not found", work_order_id))
            })?;

        // Can only resume work orders that are ON_HOLD
        if work_order.status_code != Some("ON_HOLD".to_string()) {
            return Err(ServiceError::InvalidOperation(
                "Can only resume work orders that are on hold".to_string(),
            ));
        }

        // Determine what status to resume to based on whether it was started
        let resume_status = if work_order.actual_start_date.is_some() {
            "IN_PROGRESS"
        } else {
            "READY"
        };

        let mut active: manufacturing_work_orders::ActiveModel = work_order.into();
        active.status_code = Set(Some(resume_status.to_string()));
        active.updated_at = Set(Utc::now().into());

        let updated = active.update(db).await.map_err(|e| {
            error!("Failed to resume work order: {}", e);
            ServiceError::db_error(e)
        })?;

        // Record metrics
        counter!("manufacturing.work_orders.resumed", 1);

        // Send event
        if let Some(sender) = &self.event_sender {
            sender
                .send_or_log(Event::WorkOrderResumed { work_order_id })
                .await;
        }

        info!("Work order {} resumed", work_order_id);

        Ok(updated)
    }
}

#[derive(Debug, Clone)]
pub struct WorkOrderStatus {
    pub work_order_id: i64,
    pub status: String,
    pub quantity_to_build: Decimal,
    pub quantity_completed: Decimal,
    pub actual_start_date: Option<NaiveDate>,
    pub actual_completion_date: Option<NaiveDate>,
}
