use async_trait::async_trait;;
use serde::{Serialize, Deserialize};
use std::sync::Arc;
use sea_orm::*;
use crate::{errors::ServiceError, db::DbPool, models::*};
use chrono::{DateTime, Utc};


use crate::billofmaterials::BillOfMaterials;
use crate::inventory_item::InventoryItem;
use crate::order::Order;
use crate::shipment::Shipment;
use crate::tracking_event::TrackingEvent;
use crate::work_order::WorkOrder;
use crate::return_entity::ReturnEntity;
use crate::order_item::OrderItem;   
use crate::product::Product;
use crate::customer::Customer;
use crate::order::Order;
use crate::warehouse::Warehouse;
use crate::manufacture_order_component_entity::ManufactureOrderComponent;
use crate::manufacture_order_operation_entity::ManufactureOrderOperation;
use crate::manufacture_order_entity::ManufactureOrder;
use crate::manufacture_order_status::ManufactureOrderStatus;


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
        let db = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;

        let locations = warehouse_inventory_entity::Entity::find()
            .join_rev(
                JoinType::InnerJoin,
                warehouse_location_entity::Entity::belongs_to(warehouse_inventory_entity::Entity)
                    .from(warehouse_inventory_entity::Column::LocationId)
                    .to(warehouse_location_entity::Column::Id)
                    .into(),
            )
            .filter(warehouse_inventory_entity::Column::ProductId.eq(self.product_id))
            .select_only()
            .column(warehouse_location_entity::Column::Id)
            .column(warehouse_location_entity::Column::Name)
            .column(warehouse_inventory_entity::Column::Quantity)
            .into_tuple()
            .all(&db)
            .await
            .map_err(|_| ServiceError::DatabaseError)?;

        Ok(locations
            .into_iter()
            .map(|(location_id, location_name, quantity)| WarehouseLocationStock {
                location_id,
                location_name,
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
        let db = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;

        let order_items = order_item_entity::Entity::find()
            .filter(order_item_entity::Column::OrderId.eq(self.order_id))
            .all(&db)
            .await
            .map_err(|_| ServiceError::DatabaseError)?;

        let mut picking_locations = Vec::new();

        for item in order_items {
            let location = warehouse_inventory_entity::Entity::find()
                .join(JoinType::InnerJoin, warehouse_location_entity::Entity)
                .join(JoinType::InnerJoin, product_entity::Entity)
                .filter(warehouse_inventory_entity::Column::ProductId.eq(item.product_id))
                .filter(warehouse_inventory_entity::Column::Quantity.gte(item.quantity))
                .order_by_asc(warehouse_location_entity::Column::PickSequence)
                .select_only()
                .column(product_entity::Column::Id)
                .column(product_entity::Column::Name)
                .column(warehouse_location_entity::Column::Id)
                .column(warehouse_location_entity::Column::Name)
                .column(warehouse_inventory_entity::Column::Quantity)
                .into_tuple()
                .one(&db)
                .await
                .map_err(|_| ServiceError::DatabaseError)?;

            if let Some((product_id, product_name, location_id, location_name, _)) = location {
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
        let db = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;

        let total_locations = warehouse_location_entity::Entity::find()
            .filter(warehouse_location_entity::Column::WarehouseId.eq(self.warehouse_id))
            .count(&db)
            .await
            .map_err(|_| ServiceError::DatabaseError)?;

        let occupied_locations = warehouse_inventory_entity::Entity::find()
            .join_rev(
                JoinType::InnerJoin,
                warehouse_location_entity::Entity::belongs_to(warehouse_inventory_entity::Entity)
                    .from(warehouse_inventory_entity::Column::LocationId)
                    .to(warehouse_location_entity::Column::Id)
                    .into(),
            )
            .filter(warehouse_location_entity::Column::WarehouseId.eq(self.warehouse_id))
            .filter(warehouse_inventory_entity::Column::Quantity.gt(0))
            .count(&db)
            .await
            .map_err(|_| ServiceError::DatabaseError)?;

        let utilization_rate = occupied_locations as f64 / total_locations as f64;

        let volume_data = warehouse_location_entity::Entity::find()
            .left_join(warehouse_inventory_entity::Entity)
            .filter(warehouse_location_entity::Column::WarehouseId.eq(self.warehouse_id))
            .select_only()
            .column_as(sum(warehouse_location_entity::Column::Volume), "total_volume")
            .column_as(
                sum(
                    warehouse_inventory_entity::Column::Quantity.cast::<f64>()
                        * product_entity::Column::Volume,
                ),
                "used_volume",
            )
            .into_tuple()
            .one(&db)
            .await
            .map_err(|_| ServiceError::DatabaseError)?;

        let total_volume = volume_data.0.unwrap_or(0.0);
        let used_volume = volume_data.1.unwrap_or(0.0);
        let volume_utilization_rate = if total_volume > 0.0 {
            used_volume / total_volume
        } else {
            0.0
        };

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
        let db = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;

        let movements = inventory_movement_entity::Entity::find()
            .left_join(warehouse_location_entity::Entity)
            .filter(inventory_movement_entity::Column::ProductId.eq(self.product_id))
            .filter(inventory_movement_entity::Column::Timestamp.between(self.start_date, self.end_date))
            .select_only()
            .column(inventory_movement_entity::Column::Id)
            .column(inventory_movement_entity::Column::MovementType)
            .column_as(warehouse_location_entity::Column::Name.nullable(), "from_location")
            .column_as(warehouse_location_entity::Column::Name.nullable(), "to_location")
            .column(inventory_movement_entity::Column::Quantity)
            .column(inventory_movement_entity::Column::Timestamp)
            .into_tuple()
            .all(&db)
            .await
            .map_err(|_| ServiceError::DatabaseError)?;

        Ok(movements
            .into_iter()
            .map(|(movement_id, movement_type, from_location, to_location, quantity, timestamp)| InventoryMovement {
                movement_id,
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
        let db = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;

        let new_cycle_count = cycle_count_entity::ActiveModel {
            location_id: Set(self.location_id.clone()),
            counter_id: Set(self.counter_id),
            status: Set(CycleCountStatus::Pending),
            created_at: Set(Utc::now()),
            ..Default::default()
        };

        let cycle_count = new_cycle_count
            .insert(&db)
            .await
            .map_err(|_| ServiceError::DatabaseError)?;

        Ok(cycle_count.into())
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
    type Result = Vec<inventory_adjustment_entity::Model>;

    async fn execute(&self, db_pool: Arc<DbPool>) -> Result<Self::Result, ServiceError> {
        let db = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;

        let mut adjustments = Vec::new();

        let transaction = db
            .transaction::<_, DbErr, _>(|txn| {
                Box::pin(async move {
                    for reconciliation in &self.reconciliations {
                        let current_quantity = warehouse_inventory_entity::Entity::find()
                            .filter(warehouse_inventory_entity::Column::ProductId.eq(reconciliation.product_id))
                            .filter(warehouse_inventory_entity::Column::LocationId.eq(
                                cycle_count_entity::Entity::find_by_id(self.cycle_count_id)
                                    .select_only()
                                    .column(cycle_count_entity::Column::LocationId)
                                    .into_tuple()
                                    .one(txn)
                                    .await?,
                            ))
                            .select_only()
                            .column(warehouse_inventory_entity::Column::Quantity)
                            .into_tuple()
                            .one(txn)
                            .await?
                            .unwrap_or(0);

                        let difference = reconciliation.counted_quantity - current_quantity;

                        if difference != 0 {
                            let adjustment = inventory_adjustment_entity::ActiveModel {
                                cycle_count_id: Set(self.cycle_count_id),
                                product_id: Set(reconciliation.product_id),
                                quantity_change: Set(difference),
                                created_at: Set(Utc::now()),
                                ..Default::default()
                            };

                            let adjustment = adjustment.insert(txn).await?;

                            warehouse_inventory_entity::Entity::update_many()
                                .filter(warehouse_inventory_entity::Column::ProductId.eq(reconciliation.product_id))
                                .filter(warehouse_inventory_entity::Column::LocationId.eq(
                                    cycle_count_entity::Entity::find_by_id(self.cycle_count_id)
                                        .select_only()
                                        .column(cycle_count_entity::Column::LocationId)
                                        .into_tuple()
                                        .one(txn)
                                        .await?,
                                ))
                                .col_expr(warehouse_inventory_entity::Column::Quantity, Expr::col(warehouse_inventory_entity::Column::Quantity).add(difference))
                                .exec(txn)
                                .await?;

                            adjustments.push(adjustment);
                        }
                    }

                    cycle_count_entity::Entity::update_many()
                        .filter(cycle_count_entity::Column::Id.eq(self.cycle_count_id))
                        .col_expr(cycle_count_entity::Column::Status, Expr::value(CycleCountStatus::Completed))
                        .exec(txn)
                        .await?;

                    Ok(adjustments)
                })
            })
            .await;

        transaction.map_err(|_| ServiceError::DatabaseError)
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
        let db = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;

        let incoming_items = incoming_shipment_item_entity::Entity::find()
            .filter(incoming_shipment_item_entity::Column::ShipmentId.eq(self.incoming_shipment_id))
            .all(&db)
            .await
            .map_err(|_| ServiceError::DatabaseError)?;

        let mut opportunities = Vec::new();

        for item in incoming_items {
            let matching_orders = order_item_entity::Entity::find()
                .inner_join(order_entity::Entity)
                .inner_join(product_entity::Entity)
                .filter(order_item_entity::Column::ProductId.eq(item.product_id))
                .filter(order_entity::Column::Status.eq(OrderStatus::Pending))
                .select_only()
                .column(product_entity::Column::Id)
                .column(product_entity::Column::Name)
                .column(order_entity::Column::Id)
                .column(order_item_entity::Column::Quantity)
                .into_tuple()
                .all(&db)
                .await
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
        let db = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;

        let pick_data = pick_task_entity::Entity::find()
            .filter(pick_task_entity::Column::CreatedAt.between(self.start_date, self.end_date))
            .filter(pick_task_entity::Column::Status.eq(PickTaskStatus::Completed))
            .select_only()
            .column_as(count_star(), "total_picks")
            .column_as(avg(pick_task_entity::Column::CompletedAt - pick_task_entity::Column::CreatedAt), "average_pick_time")
            .into_tuple()
            .one(&db)
            .await
            .map_err(|_| ServiceError::DatabaseError)?;

        let total_picks = pick_data.0.unwrap_or(0);
        let average_pick_time = pick_data.1.unwrap_or(0.0);
        let picks_per_hour = if average_pick_time > 0.0 {
            3600.0 / average_pick_time
        } else {
            0.0
        };

        let picker_efficiency = pick_task_entity::Entity::find()
            .inner_join(user_entity::Entity)
            .filter(pick_task_entity::Column::CreatedAt.between(self.start_date, self.end_date))
            .filter(pick_task_entity::Column::Status.eq(PickTaskStatus::Completed))
            .group_by(user_entity::Column::Id)
            .select_only()
            .column(user_entity::Column::Name)
            .column_as(avg(pick_task_entity::Column::CompletedAt - pick_task_entity::Column::CreatedAt), "average_time")
            .into_tuple()
            .all(&db)
            .await
            .map_err(|_| ServiceError::DatabaseError)?;

        let (most_efficient_picker, least_efficient_picker) = picker_efficiency
            .iter()
            .fold(
                (String::new(), String::new(), f64::MAX, f64::MIN),
                |acc, (name, time)| {
                    let time = time.unwrap_or(f64::MAX);
                    (
                        if time < acc.2 { name.clone() } else { acc.0 },
                        if time > acc.3 { name.clone() } else { acc.1 },
                        time.min(acc.2),
                        time.max(acc.3),
                    )
                },
            )
            .into();

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
        let db = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;

        let replenishment_needs = warehouse_inventory_entity::Entity::find()
            .join(JoinType::InnerJoin, warehouse_location_entity::Entity)
            .join(JoinType::InnerJoin, product_entity::Entity)
            .filter(
                Expr::col(warehouse_inventory_entity::Column::Quantity)
                    .div(Expr::col(warehouse_location_entity::Column::Capacity).cast::<f64>())
                    .lte(self.threshold_percentage),
            )
            .select_only()
            .column(warehouse_location_entity::Column::Id)
            .column(warehouse_location_entity::Column::Name)
            .column(product_entity::Column::Id)
            .column(product_entity::Column::Name)
            .column(warehouse_inventory_entity::Column::Quantity)
            .column(warehouse_location_entity::Column::Capacity)
            .into_tuple()
            .all(&db)
            .await
            .map_err(|_| ServiceError::DatabaseError)?;

        Ok(replenishment_needs
            .into_iter()
            .map(
                |(location_id, location_name, product_id, product_name, current_quantity, bin_capacity)| {
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
                },
            )
            .collect())
    }
}
