use async_trait::async_trait;
use serde::{Serialize, Deserialize};
use std::sync::Arc;
use crate::{errors::ServiceError, db::DbPool, models::*};
use chrono::{DateTime, Utc};
use diesel::prelude::*;
use diesel::dsl::*;

#[async_trait]
pub trait Query: Send + Sync {
    type Result: Send + Sync;

    async fn execute(&self, db_pool: Arc<DbPool>) -> Result<Self::Result, ServiceError>;
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetWarehouseLocationsByProductQuery {
    pub product_id: i32,
}

#[derive(Debug, Serialize)]
pub struct WarehouseLocationStock {
    pub location_id: String,
    pub location_name: String,
    pub quantity: i32,
}

#[async_trait]
impl Query for GetWarehouseLocationsByProductQuery {
    type Result = Vec<WarehouseLocationStock>;

    async fn execute(&self, db_pool: Arc<DbPool>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;
        let locations = warehouse_inventory::table
            .inner_join(warehouse_locations::table)
            .filter(warehouse_inventory::product_id.eq(self.product_id))
            .select((
                warehouse_locations::id,
                warehouse_locations::name,
                warehouse_inventory::quantity,
            ))
            .load::<(String, String, i32)>(&conn)
            .map_err(|_| ServiceError::DatabaseError)?;

        Ok(locations
            .into_iter()
            .map(|(id, name, quantity)| WarehouseLocationStock {
                location_id: id,
                location_name: name,
                quantity,
            })
            .collect())
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetOptimalPickingLocationsQuery {
    pub order_id: i32,
}

#[derive(Debug, Serialize)]
pub struct PickingLocation {
    pub product_id: i32,
    pub product_name: String,
    pub location_id: String,
    pub location_name: String,
    pub quantity_to_pick: i32,
}

#[async_trait]
impl Query for GetOptimalPickingLocationsQuery {
    type Result = Vec<PickingLocation>;

    async fn execute(&self, db_pool: Arc<DbPool>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;
        
        // First, get the order items
        let order_items = order_items::table
            .filter(order_items::order_id.eq(self.order_id))
            .load::<OrderItem>(&conn)
            .map_err(|_| ServiceError::DatabaseError)?;

        let mut picking_locations = Vec::new();

        for item in order_items {
            let locations = warehouse_inventory::table
                .inner_join(warehouse_locations::table)
                .inner_join(products::table.on(warehouse_inventory::product_id.eq(products::id)))
                .filter(warehouse_inventory::product_id.eq(item.product_id))
                .filter(warehouse_inventory::quantity.ge(item.quantity))
                .order(warehouse_locations::pick_sequence.asc())
                .select((
                    products::id,
                    products::name,
                    warehouse_locations::id,
                    warehouse_locations::name,
                    warehouse_inventory::quantity,
                ))
                .first::<(i32, String, String, String, i32)>(&conn)
                .optional()
                .map_err(|_| ServiceError::DatabaseError)?;

            if let Some((product_id, product_name, location_id, location_name, _)) = locations {
                picking_locations.push(PickingLocation {
                    product_id,
                    product_name,
                    location_id,
                    location_name,
                    quantity_to_pick: item.quantity,
                });
            }
        }

        Ok(picking_locations)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetWarehouseUtilizationQuery {
    pub warehouse_id: i32,
}

#[derive(Debug, Serialize)]
pub struct WarehouseUtilization {
    pub total_locations: i32,
    pub occupied_locations: i32,
    pub utilization_rate: f64,
    pub total_volume: f64,
    pub used_volume: f64,
    pub volume_utilization_rate: f64,
}

#[async_trait]
impl Query for GetWarehouseUtilizationQuery {
    type Result = WarehouseUtilization;

    async fn execute(&self, db_pool: Arc<DbPool>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;
        
        let total_locations: i32 = warehouse_locations::table
            .filter(warehouse_locations::warehouse_id.eq(self.warehouse_id))
            .count()
            .get_result(&conn)
            .map_err(|_| ServiceError::DatabaseError)?;

        let occupied_locations: i32 = warehouse_inventory::table
            .inner_join(warehouse_locations::table)
            .filter(warehouse_locations::warehouse_id.eq(self.warehouse_id))
            .filter(warehouse_inventory::quantity.gt(0))
            .select(count_distinct(warehouse_locations::id))
            .first(&conn)
            .map_err(|_| ServiceError::DatabaseError)?;

        let utilization_rate = occupied_locations as f64 / total_locations as f64;

        let volume_data: (Option<f64>, Option<f64>) = warehouse_locations::table
            .left_join(warehouse_inventory::table)
            .filter(warehouse_locations::warehouse_id.eq(self.warehouse_id))
            .select((
                sum(warehouse_locations::volume),
                sum(warehouse_inventory::quantity.cast::<f64>() * products::volume),
            ))
            .first(&conn)
            .map_err(|_| ServiceError::DatabaseError)?;

        let total_volume = volume_data.0.unwrap_or(0.0);
        let used_volume = volume_data.1.unwrap_or(0.0);
        let volume_utilization_rate = if total_volume > 0.0 { used_volume / total_volume } else { 0.0 };

        Ok(WarehouseUtilization {
            total_locations,
            occupied_locations,
            utilization_rate,
            total_volume,
            used_volume,
            volume_utilization_rate,
        })
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetInventoryMovementHistoryQuery {
    pub product_id: i32,
    pub start_date: DateTime<Utc>,
    pub end_date: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct InventoryMovement {
    pub movement_id: i32,
    pub movement_type: String,
    pub from_location: Option<String>,
    pub to_location: Option<String>,
    pub quantity: i32,
    pub timestamp: DateTime<Utc>,
}

#[async_trait]
impl Query for GetInventoryMovementHistoryQuery {
    type Result = Vec<InventoryMovement>;

    async fn execute(&self, db_pool: Arc<DbPool>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;
        
        let movements = inventory_movements::table
            .left_join(warehouse_locations::table.on(inventory_movements::from_location_id.eq(warehouse_locations::id.nullable())))
            .left_join(warehouse_locations::table.on(inventory_movements::to_location_id.eq(warehouse_locations::id.nullable())))
            .filter(inventory_movements::product_id.eq(self.product_id))
            .filter(inventory_movements::timestamp.between(self.start_date, self.end_date))
            .select((
                inventory_movements::id,
                inventory_movements::movement_type,
                warehouse_locations::name.nullable().first(),
                warehouse_locations::name.nullable().second(),
                inventory_movements::quantity,
                inventory_movements::timestamp,
            ))
            .load::<(i32, String, Option<String>, Option<String>, i32, DateTime<Utc>)>(&conn)
            .map_err(|_| ServiceError::DatabaseError)?;

        Ok(movements
            .into_iter()
            .map(|(id, movement_type, from_location, to_location, quantity, timestamp)| InventoryMovement {
                movement_id: id,
                movement_type,
                from_location,
                to_location,
                quantity,
                timestamp,
            })
            .collect())
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateCycleCountQuery {
    pub location_id: String,
    pub counter_id: i32,
}

#[derive(Debug, Serialize)]
pub struct CycleCount {
    pub id: i32,
    pub location_id: String,
    pub counter_id: i32,
    pub status: CycleCountStatus,
    pub created_at: DateTime<Utc>,
}

#[async_trait]
impl Query for CreateCycleCountQuery {
    type Result = CycleCount;

    async fn execute(&self, db_pool: Arc<DbPool>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;
        
        let new_cycle_count = NewCycleCount {
            location_id: &self.location_id,
            counter_id: self.counter_id,
            status: CycleCountStatus::Pending,
            created_at: Utc::now(),
        };

        let cycle_count = diesel::insert_into(cycle_counts::table)
            .values(&new_cycle_count)
            .get_result::<CycleCount>(&conn)
            .map_err(|_| ServiceError::DatabaseError)?;

        Ok(cycle_count)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ReconcileInventoryQuery {
    pub cycle_count_id: i32,
    pub reconciliations: Vec<InventoryReconciliation>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct InventoryReconciliation {
    pub product_id: i32,
    pub counted_quantity: i32,
}

#[async_trait]
impl Query for ReconcileInventoryQuery {
    type Result = Vec<InventoryAdjustment>;

    async fn execute(&self, db_pool: Arc<DbPool>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;
        
        let mut adjustments = Vec::new();

        conn.transaction::<_, diesel::result::Error, _>(|| {
            for reconciliation in &self.reconciliations {
                let current_quantity: i32 = warehouse_inventory::table
                    .filter(warehouse_inventory::product_id.eq(reconciliation.product_id))
                    .filter(warehouse_inventory::location_id.eq(cycle_counts::table.select(cycle_counts::location_id).find(self.cycle_count_id)))
                    .select(warehouse_inventory::quantity)
                    .first(&conn)?;

                let difference = reconciliation.counted_quantity - current_quantity;

                if difference != 0 {
                    let adjustment = diesel::insert_into(inventory_adjustments::table)
                        .values((
                            inventory_adjustments::cycle_count_id.eq(self.cycle_count_id),
                            inventory_adjustments::product_id.eq(reconciliation.product_id),
                            inventory_adjustments::quantity_change.eq(difference),
                            inventory_adjustments::created_at.eq(Utc::now()),
                        ))
                        .get_result::<InventoryAdjustment>(&conn)?;

                    diesel::update(warehouse_inventory::table)
                        .filter(warehouse_inventory::product_id.eq(reconciliation.product_id))
                        .filter(warehouse_inventory::location_id.eq(cycle_counts::table.select(cycle_counts::location_id).find(self.cycle_count_id)))
                        .set(warehouse_inventory::quantity.eq(warehouse_inventory::quantity + difference))
                        .execute(&conn)?;

                    adjustments.push(adjustment);
                }
            }

            diesel::update(cycle_counts::table)
                .filter(cycle_counts::id.eq(self.cycle_count_id))
                .set(cycle_counts::status.eq(CycleCountStatus::Completed))
                .execute(&conn)?;

            Ok(())
        }).map_err(|_| ServiceError::DatabaseError)?;

        Ok(adjustments)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetCrossDockingOpportunitiesQuery {
    pub incoming_shipment_id: i32,
}

#[derive(Debug, Serialize)]
pub struct CrossDockingOpportunity {
    pub product_id: i32,
    pub product_name: String,
    pub incoming_quantity: i32,
    pub outgoing_order_id: i32,
    pub outgoing_quantity: i32,
}

#[async_trait]
impl Query for GetCrossDockingOpportunitiesQuery {
    type Result = Vec<CrossDockingOpportunity>;

    async fn execute(&self, db_pool: Arc<DbPool>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;
        
        let incoming_items = incoming_shipment_items::table
            .filter(incoming_shipment_items::shipment_id.eq(self.incoming_shipment_id))
            .load::<IncomingShipmentItem>(&conn)
            .map_err(|_| ServiceError::DatabaseError)?;

        let mut opportunities = Vec::new();

        for item in incoming_items {
            let matching_orders = order_items::table
                .inner_join(orders::table)
                .inner_join(products::table.on(order_items::product_id.eq(products::id)))
                .filter(order_items::product_id.eq(item.product_id))
                .filter(orders::status.eq(OrderStatus::Pending))
                .select((
                    products::id,
                    products::name,
                    orders::id,
                    order_items::quantity,
                ))
                .load::<(i32, String, i32, i32)>(&conn)
                .map_err(|_| ServiceError::DatabaseError)?;

            for (product_id, product_name, order_id, outgoing_quantity) in matching_orders {
                opportunities.push(CrossDockingOpportunity {
                    product_id,
                    product_name,
                    incoming_quantity: item.quantity,
                    outgoing_order_id: order_id,
                    outgoing_quantity,
                });
            }
        }

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
    pub total_picks: i32,
    pub average_pick_time: f64,
    pub picks_per_hour: f64,
    pub most_efficient_picker: String,
    pub least_efficient_picker: String,
}

#[async_trait]
impl Query for AnalyzePickEfficiencyQuery {
    type Result = PickEfficiencyAnalysis;

    async fn execute(&self, db_pool: Arc<DbPool>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;
        
        let pick_data: (i32, Option<f64>) = pick_tasks::table
            .filter(pick_tasks::created_at.between(self.start_date, self.end_date))
            .filter(pick_tasks::status.eq(PickTaskStatus::Completed))
            .select((
                count_star(),
                avg(pick_tasks::completed_at - pick_tasks::created_at),
            ))
            .first(&conn)
            .map_err(|_| ServiceError::DatabaseError)?;

        let total_picks = pick_data.0;
        let average_pick_time = pick_data.1.unwrap_or(0.0);
        let picks_per_hour = if average_pick_time > 0.0 { 3600.0 / average_pick_time } else { 0.0 };

        let picker_efficiency: Vec<(String, f64)> = pick_tasks::table
            .inner_join(users::table)
            .filter(pick_tasks::created_at.between(self.start_date, self.end_date))
            .filter(pick_tasks::status.eq(PickTaskStatus::Completed))
            .group_by(users::id)
            .select((
                users::name,
                avg(pick_tasks::completed_at - pick_tasks::created_at),
            ))
            .load(&conn)
            .map_err(|_| ServiceError::DatabaseError)?;

        let (most_efficient_picker, least_efficient_picker) = picker_efficiency.iter()
            .fold((String::new(), String::new()), |acc, (name, time)| {
                if acc.0.is_empty() || time < &acc.1.parse::<f64>().unwrap_or(f64::MAX) {
                    (name.clone(), time.to_string())
                } else if acc.1.is_empty() || time > &acc.1.parse::<f64>().unwrap_or(0.0) {
                    (acc.0, name.clone())
                } else {
                    acc
                }
            });

        Ok(PickEfficiencyAnalysis {
            total_picks,
            average_pick_time,
            picks_per_hour,
            most_efficient_picker,
            least_efficient_picker,
        })
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetBinReplenishmentNeedsQuery {
    pub threshold_percentage: f64,
}

#[derive(Debug, Serialize)]
pub struct BinReplenishmentNeed {
    pub location_id: String,
    pub location_name: String,
    pub product_id: i32,
    pub product_name: String,
    pub current_quantity: i32,
    pub bin_capacity: i32,
    pub replenishment_quantity: i32,
}

#[async_trait]
impl Query for GetBinReplenishmentNeedsQuery {
    type Result = Vec<BinReplenishmentNeed>;

    async fn execute(&self, db_pool: Arc<DbPool>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;
        
        let replenishment_needs = warehouse_inventory::table
            .inner_join(warehouse_locations::table)
            .inner_join(products::table)
            .filter(warehouse_inventory::quantity.cast::<f64>() / warehouse_locations::capacity.cast::<f64>().le(self.threshold_percentage))
            .select((
                warehouse_locations::id,
                warehouse_locations::name,
                products::id,
                products::name,
                warehouse_inventory::quantity,
                warehouse_locations::capacity,
            ))
            .load::<(String, String, i32, String, i32, i32)>(&conn)
            .map_err(|_| ServiceError::DatabaseError)?;

        Ok(replenishment_needs
            .into_iter()
            .map(|(location_id, location_name, product_id, product_name, current_quantity, bin_capacity)| {
                let replenishment_quantity = bin_capacity - current_quantity;
                BinReplenishmentNeed {
                    location_id,
                    location_name,
                    product_id,
                    product_name,
                    current_quantity,
                    bin_capacity,
                    replenishment_quantity,
                }
            })
            .collect())
    }
}