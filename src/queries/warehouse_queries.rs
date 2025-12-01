use crate::{db::DbPool, errors::ServiceError, models::*};
use crate::models::{
    warehouse_location_entity::{Entity as WarehouseLocationEntity},
    product::{Entity as ProductEntity},
    cyclecounts::{Entity as CycleCountEntity, Model as CycleCountModel, ActiveModel as CycleCountActiveModel, CycleCountStatus},
    inventory_item_entity::{Entity as InventoryItemEntity, Column as InventoryItemColumn},
};
use crate::entities::inventory_adjustment_entity::{Entity as InventoryAdjustmentEntity, Model as InventoryAdjustmentModel, ActiveModel as InventoryAdjustmentActiveModel};
// Note: Some entities are not yet available in the models module
// These will be commented out until they are implemented
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sea_orm::{
    prelude::*, query::*, EntityTrait, QueryFilter, QueryOrder, QuerySelect, RelationTrait,
    DatabaseConnection, Set,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

#[async_trait]
pub trait Query: Send + Sync {
    type Result: Send + Sync;
    async fn execute(&self, db_pool: &DatabaseConnection) -> Result<Self::Result, ServiceError>;
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetWarehouseLocationQuery {
    pub location_id: Uuid,
}

#[async_trait]
impl Query for GetWarehouseLocationQuery {
    type Result = warehouse_location_entity::Model;

    async fn execute(&self, db_pool: &DatabaseConnection) -> Result<Self::Result, ServiceError> {
        WarehouseLocationEntity::find_by_id(self.location_id)
            .one(db_pool)
            .await
            .map_err(|e| ServiceError::db_error(e))?
            .ok_or_else(|| ServiceError::NotFound("Warehouse location not found".to_string()))
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ReconcileInventoryQuery {
    pub cycle_count_id: Uuid,
    pub reconciliations: Vec<InventoryReconciliation>,
    pub user_id: Uuid,
    pub warehouse_id: Uuid,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct InventoryReconciliation {
    pub product_id: Uuid,
    pub counted_quantity: f64,
    pub reason: String,
}

#[async_trait]
impl Query for ReconcileInventoryQuery {
    type Result = Vec<InventoryAdjustmentModel>;

    async fn execute(&self, db_pool: &DatabaseConnection) -> Result<Self::Result, ServiceError> {
        let mut adjustments = Vec::new();

        for reconciliation in &self.reconciliations {
            // Get the inventory item for the product
            let inventory_item = InventoryItemEntity::find()
                .filter(InventoryItemColumn::ProductId.eq(reconciliation.product_id))
                .one(db_pool)
                .await
                .map_err(ServiceError::from)?
                .ok_or_else(|| ServiceError::NotFound(
                    format!("Inventory item not found for product {}", reconciliation.product_id)
                ))?;

            // Calculate adjustment quantity (difference between counted and system quantity)
            let adjustment_quantity = reconciliation.counted_quantity as i32 - inventory_item.quantity_on_hand;

            if adjustment_quantity != 0 {
                // Create inventory adjustment
                let adjustment = InventoryAdjustmentActiveModel {
                    id: Set(Uuid::new_v4()),
                    reference_number: Set(Some(format!("CC-{}", self.cycle_count_id))),
                    reason: Set(reconciliation.reason.clone()),
                    quantity: Set(adjustment_quantity),
                    unit_cost: Set(inventory_item.unit_cost),
                    total_cost: Set(inventory_item.unit_cost.map(|cost| cost * rust_decimal::Decimal::from(adjustment_quantity.abs()))),
                    location_id: Set(Some(self.warehouse_id)),
                    inventory_item_id: Set(inventory_item.id),
                    created_by: Set(Some(self.user_id.to_string())),
                    approved_by: Set(None),
                    created_at: Set(Utc::now()),
                    updated_at: Set(Utc::now()),
                };

                let inserted = adjustment.insert(db_pool).await.map_err(ServiceError::from)?;
                adjustments.push(inserted);
            }
        }

        // Update cycle count status to completed
        let cycle_count = CycleCountEntity::find_by_id(self.cycle_count_id)
            .one(db_pool)
            .await
            .map_err(ServiceError::from)?
            .ok_or_else(|| ServiceError::NotFound("Cycle count not found".to_string()))?;

        let mut cycle_count_active: CycleCountActiveModel = cycle_count.into();
        cycle_count_active.status = Set(CycleCountStatus::Completed);
        cycle_count_active.completed_at = Set(Some(Utc::now()));
        cycle_count_active.update(db_pool).await.map_err(ServiceError::from)?;

        Ok(adjustments)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetCrossDockingOpportunitiesQuery {
    pub incoming_shipment_id: i32,
}

#[derive(Debug, Serialize)]
pub struct CrossDockingOpportunity {
    pub product_id: Uuid,
    pub product_name: String,
    pub incoming_quantity: f64,
    pub outgoing_order_id: Uuid,
    pub outgoing_quantity: f64,
}

#[async_trait]
impl Query for GetCrossDockingOpportunitiesQuery {
    type Result = Vec<CrossDockingOpportunity>;

    async fn execute(&self, _db_pool: &DatabaseConnection) -> Result<Self::Result, ServiceError> {
        // TODO: Implement when incoming_shipment_item_entity is available
        let opportunities = Vec::new();
        Ok(opportunities)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AnalyzePickEfficiencyQuery {
    pub start_date: DateTime<Utc>,
    pub end_date: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct PickEfficiencyAnalysis {
    pub total_picks: u64,
    pub average_pick_time: f64,
    pub accuracy_rate: f64,
    pub most_efficient_picker: Option<String>,
    pub least_efficient_picker: Option<String>,
    pub picker_efficiency: Vec<PickerEfficiency>,
}

#[derive(Debug, Serialize)]
pub struct PickerEfficiency {
    pub user_id: Uuid,
    pub user_name: String,
    pub total_picks: u64,
    pub average_pick_time: f64,
    pub accuracy_rate: f64,
}

#[async_trait]
impl Query for AnalyzePickEfficiencyQuery {
    type Result = PickEfficiencyAnalysis;

    async fn execute(&self, _db_pool: &DatabaseConnection) -> Result<Self::Result, ServiceError> {
        // TODO: Implement when pick entities are available
        let analysis = PickEfficiencyAnalysis {
            total_picks: 0,
            average_pick_time: 0.0,
            accuracy_rate: 0.0,
            most_efficient_picker: None,
            least_efficient_picker: None,
            picker_efficiency: Vec::new(),
        };
        Ok(analysis)
    }
}