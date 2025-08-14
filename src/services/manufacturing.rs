use std::sync::Arc;
use chrono::{Utc, NaiveDate};
use rust_decimal::Decimal;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, DatabaseConnection, EntityTrait, TransactionTrait,
    QueryFilter, ColumnTrait,
};
use tracing::{error, info, instrument, warn};

use crate::{
    entities::{
        manufacturing_work_orders::{self, Entity as WorkOrderEntity},
        bom_header::{self, Entity as BomHeaderEntity},
        item_master::{self, Entity as ItemMasterEntity},
    },
    errors::ServiceError,
    services::{
        inventory_sync::{InventorySyncService, TransactionType},
        bom::BomService,
    },
    events::{Event, EventSender},
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
        let db = &*self.db;
        let txn = db.begin().await.map_err(|e| ServiceError::DatabaseError(e))?;

        // Verify item exists and has a BOM
        let item = ItemMasterEntity::find_by_id(item_id)
            .one(&txn)
            .await
            .map_err(|e| ServiceError::DatabaseError(e))?
            .ok_or_else(|| ServiceError::NotFound(format!("Item {} not found", item_id)))?;

        // Find active BOM for the item
        let bom = BomHeaderEntity::find()
            .filter(bom_header::Column::ItemId.eq(item_id))
            .filter(bom_header::Column::StatusCode.eq("ACTIVE"))
            .one(&txn)
            .await
            .map_err(|e| ServiceError::DatabaseError(e))?
            .ok_or_else(|| ServiceError::NotFound(format!("No active BOM found for item {}", item_id)))?;

        // Validate component availability
        let availability = self.bom_service
            .validate_component_availability(bom.bom_id, quantity_to_build, location_id)
            .await?;

        if !availability.can_produce {
            warn!("Insufficient components for work order: {:?}", availability.shortages);
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
            ServiceError::DatabaseError(e)
        })?;

        txn.commit().await.map_err(|e| ServiceError::DatabaseError(e))?;

        // Send event
        if let Some(sender) = &self.event_sender {
            let _ = sender.send(Event::WorkOrderCreated {
                work_order_id: created.work_order_id,
                item_id,
                quantity: quantity_to_build,
            }).await;
        }

        info!("Work order created: {} for item {} quantity {}", 
            work_order_number, item_id, quantity_to_build);

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
        let txn = db.begin().await.map_err(|e| ServiceError::DatabaseError(e))?;

        // Get work order
        let work_order = WorkOrderEntity::find_by_id(work_order_id)
            .one(&txn)
            .await
            .map_err(|e| ServiceError::DatabaseError(e))?
            .ok_or_else(|| ServiceError::NotFound(format!("Work order {} not found", work_order_id)))?;

        // Validate status
        if work_order.status_code != Some("READY".to_string()) {
            return Err(ServiceError::InvalidOperation(
                format!("Work order {} is not ready to start. Current status: {:?}", 
                    work_order_id, work_order.status_code)
            ));
        }

        let item_id = work_order.item_id
            .ok_or_else(|| ServiceError::InvalidOperation("Work order has no item".to_string()))?;
        let quantity = work_order.quantity_to_build
            .ok_or_else(|| ServiceError::InvalidOperation("Work order has no quantity".to_string()))?;

        // Find BOM
        let bom = BomHeaderEntity::find()
            .filter(bom_header::Column::ItemId.eq(item_id))
            .filter(bom_header::Column::StatusCode.eq("ACTIVE"))
            .one(&txn)
            .await
            .map_err(|e| ServiceError::DatabaseError(e))?
            .ok_or_else(|| ServiceError::NotFound(format!("No active BOM found for item {}", item_id)))?;

        // Consume components
        self.bom_service
            .consume_components_for_production(bom.bom_id, quantity, location_id, work_order_id)
            .await?;

        // Update work order status
        let mut active: manufacturing_work_orders::ActiveModel = work_order.into();
        active.status_code = Set(Some("IN_PROGRESS".to_string()));
        active.actual_start_date = Set(Some(Utc::now().date_naive()));
        active.updated_at = Set(Utc::now().into());

        let updated = active.update(&txn).await.map_err(|e| {
            error!("Failed to update work order: {}", e);
            ServiceError::DatabaseError(e)
        })?;

        txn.commit().await.map_err(|e| ServiceError::DatabaseError(e))?;

        // Send event
        if let Some(sender) = &self.event_sender {
            let _ = sender.send(Event::WorkOrderStarted {
                work_order_id,
                item_id,
            }).await;
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
        let db = &*self.db;
        let txn = db.begin().await.map_err(|e| ServiceError::DatabaseError(e))?;

        // Get work order
        let work_order = WorkOrderEntity::find_by_id(work_order_id)
            .one(&txn)
            .await
            .map_err(|e| ServiceError::DatabaseError(e))?
            .ok_or_else(|| ServiceError::NotFound(format!("Work order {} not found", work_order_id)))?;

        // Validate status
        if work_order.status_code != Some("IN_PROGRESS".to_string()) {
            return Err(ServiceError::InvalidOperation(
                format!("Work order {} is not in progress. Current status: {:?}", 
                    work_order_id, work_order.status_code)
            ));
        }

        let item_id = work_order.item_id
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
            ServiceError::DatabaseError(e)
        })?;

        txn.commit().await.map_err(|e| ServiceError::DatabaseError(e))?;

        // Send event
        if let Some(sender) = &self.event_sender {
            let _ = sender.send(Event::WorkOrderCompleted {
                work_order_id,
                item_id,
                quantity_completed: completed_quantity,
            }).await;
        }

        info!("Work order {} completed with quantity {}", work_order_id, completed_quantity);

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
            .map_err(|e| ServiceError::DatabaseError(e))?
            .ok_or_else(|| ServiceError::NotFound(format!("Work order {} not found", work_order_id)))?;

        Ok(WorkOrderStatus {
            work_order_id,
            status: work_order.status_code.unwrap_or_else(|| "UNKNOWN".to_string()),
            quantity_to_build: work_order.quantity_to_build.unwrap_or(Decimal::ZERO),
            quantity_completed: work_order.quantity_completed.unwrap_or(Decimal::ZERO),
            actual_start_date: work_order.actual_start_date,
            actual_completion_date: work_order.actual_completion_date,
        })
    }

    /// Cancels a work order (if not started)
    #[instrument(skip(self))]
    pub async fn cancel_work_order(
        &self,
        work_order_id: i64,
    ) -> Result<(), ServiceError> {
        let db = &*self.db;
        
        let work_order = WorkOrderEntity::find_by_id(work_order_id)
            .one(db)
            .await
            .map_err(|e| ServiceError::DatabaseError(e))?
            .ok_or_else(|| ServiceError::NotFound(format!("Work order {} not found", work_order_id)))?;

        // Can only cancel if not started
        if work_order.actual_start_date.is_some() {
            return Err(ServiceError::InvalidOperation(
                "Cannot cancel work order that has already started".to_string()
            ));
        }

        let mut active: manufacturing_work_orders::ActiveModel = work_order.into();
        active.status_code = Set(Some("CANCELLED".to_string()));
        active.updated_at = Set(Utc::now().into());

        active.update(db).await.map_err(|e| {
            error!("Failed to cancel work order: {}", e);
            ServiceError::DatabaseError(e)
        })?;

        info!("Work order {} cancelled", work_order_id);

        Ok(())
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