use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use sea_orm::{
    query::{Condition, Expr, Function, QuerySelect, QueryFilter, QueryOrder},
    EntityTrait, RelationTrait, DatabaseConnection,
};
use chrono::{DateTime, Utc};

use crate::{
    errors::ServiceError,
    db::DbPool,
    models::*,
    billofmaterials::BillOfMaterials,
    inventory_item::InventoryItem,
    order::Order,
    shipment::Shipment,
    shipment_item::ShipmentItem,
    carrier::Carrier,
    tracking_event::TrackingEvent,
    work_order::WorkOrder,
    return_entity::ReturnEntity,
    order_item::OrderItem,
    product::Product,
    customer::Customer,
    warehouse::Warehouse,
    manufacture_order_component_entity::ManufactureOrderComponent,
    manufacture_order_operation_entity::ManufactureOrderOperation,
    manufacture_order_entity::ManufactureOrder,
    manufacture_order_status::ManufactureOrderStatus,
    work_order_status::WorkOrderStatus,
};

/// Trait representing a generic asynchronous query.
#[async_trait]
pub trait Query: Send + Sync {
    type Result: Send + Sync;

    /// Executes the query using the provided database pool.
    async fn execute(&self, db_pool: Arc<DbPool>) -> Result<Self::Result, ServiceError>;
}

/// Helper function to obtain a database connection from the pool.
async fn get_db(pool: &Arc<DbPool>) -> Result<DatabaseConnection, ServiceError> {
    pool.get()
        .await
        .map_err(|e| {
            log::error!("Failed to get DB connection: {:?}", e);
            ServiceError::DatabaseError
        })
}

/// Struct to get a specific shipment by ID.
#[derive(Debug, Serialize, Deserialize)]
pub struct GetShipmentByIdQuery {
    pub shipment_id: i32,
}

#[async_trait]
impl Query for GetShipmentByIdQuery {
    type Result = Shipment;

    async fn execute(&self, db_pool: Arc<DbPool>) -> Result<Self::Result, ServiceError> {
        let db = get_db(&db_pool).await?;

        Shipment::find_by_id(self.shipment_id)
            .one(&db)
            .await
            .map_err(|e| {
                log::error!("Database error in GetShipmentByIdQuery: {:?}", e);
                ServiceError::DatabaseError
            })?
            .ok_or(ServiceError::NotFound)
    }
}

/// Struct to get all shipments with a specific status, with pagination.
#[derive(Debug, Serialize, Deserialize)]
pub struct GetShipmentsByStatusQuery {
    pub status: ShipmentStatus,
    pub limit: u64,
    pub offset: u64,
}

#[async_trait]
impl Query for GetShipmentsByStatusQuery {
    type Result = Vec<Shipment>;

    async fn execute(&self, db_pool: Arc<DbPool>) -> Result<Self::Result, ServiceError> {
        let db = get_db(&db_pool).await?;

        Shipment::find()
            .filter(Shipment::Column::Status.eq(self.status))
            .order_by_desc(Shipment::Column::CreatedAt)
            .limit(self.limit)
            .offset(self.offset)
            .all(&db)
            .await
            .map_err(|e| {
                log::error!("Database error in GetShipmentsByStatusQuery: {:?}", e);
                ServiceError::DatabaseError
            })
    }
}

/// Struct to get shipments within a specific date range, with pagination.
#[derive(Debug, Serialize, Deserialize)]
pub struct GetShipmentsInDateRangeQuery {
    pub start_date: DateTime<Utc>,
    pub end_date: DateTime<Utc>,
    pub limit: u64,
    pub offset: u64,
}

#[async_trait]
impl Query for GetShipmentsInDateRangeQuery {
    type Result = Vec<Shipment>;

    async fn execute(&self, db_pool: Arc<DbPool>) -> Result<Self::Result, ServiceError> {
        let db = get_db(&db_pool).await?;

        Shipment::find()
            .filter(Shipment::Column::CreatedAt.between(self.start_date, self.end_date))
            .order_by_desc(Shipment::Column::CreatedAt)
            .limit(self.limit)
            .offset(self.offset)
            .all(&db)
            .await
            .map_err(|e| {
                log::error!("Database error in GetShipmentsInDateRangeQuery: {:?}", e);
                ServiceError::DatabaseError
            })
    }
}

/// Struct to get detailed information about a specific shipment.
#[derive(Debug, Serialize, Deserialize)]
pub struct GetShipmentDetailsQuery {
    pub shipment_id: i32,
}

/// Struct representing detailed information of a shipment.
#[derive(Debug, Serialize)]
pub struct ShipmentDetails {
    pub shipment: Shipment,
    pub order: Order,
    pub items: Vec<ShipmentItem>,
    pub tracking_events: Vec<TrackingEvent>,
}

#[async_trait]
impl Query for GetShipmentDetailsQuery {
    type Result = ShipmentDetails;

    async fn execute(&self, db_pool: Arc<DbPool>) -> Result<Self::Result, ServiceError> {
        let db = get_db(&db_pool).await?;

        // Fetch the shipment
        let shipment = Shipment::find_by_id(self.shipment_id)
            .one(&db)
            .await
            .map_err(|e| {
                log::error!("Database error fetching shipment in GetShipmentDetailsQuery: {:?}", e);
                ServiceError::DatabaseError
            })?
            .ok_or(ServiceError::NotFound)?;

        // Fetch the associated order
        let order = Order::find_by_id(shipment.order_id)
            .one(&db)
            .await
            .map_err(|e| {
                log::error!("Database error fetching order in GetShipmentDetailsQuery: {:?}", e);
                ServiceError::DatabaseError
            })?
            .ok_or(ServiceError::NotFound)?;

        // Fetch shipment items
        let items = ShipmentItem::find()
            .filter(ShipmentItem::Column::ShipmentId.eq(self.shipment_id))
            .all(&db)
            .await
            .map_err(|e| {
                log::error!("Database error fetching shipment items in GetShipmentDetailsQuery: {:?}", e);
                ServiceError::DatabaseError
            })?;

        // Fetch tracking events
        let tracking_events = TrackingEvent::find()
            .filter(TrackingEvent::Column::ShipmentId.eq(self.shipment_id))
            .order_by_desc(TrackingEvent::Column::Timestamp)
            .all(&db)
            .await
            .map_err(|e| {
                log::error!("Database error fetching tracking events in GetShipmentDetailsQuery: {:?}", e);
                ServiceError::DatabaseError
            })?;

        Ok(ShipmentDetails {
            shipment,
            order,
            items,
            tracking_events,
        })
    }
}

/// Struct to get performance metrics for shipments within a date range.
#[derive(Debug, Serialize, Deserialize)]
pub struct GetShipmentPerformanceMetricsQuery {
    pub start_date: DateTime<Utc>,
    pub end_date: DateTime<Utc>,
}

/// Struct representing shipment performance metrics.
#[derive(Debug, Serialize)]
pub struct ShipmentPerformanceMetrics {
    pub total_shipments: i64,
    pub on_time_shipments: i64,
    pub average_transit_time: f64,      // in hours
    pub average_delivery_accuracy: f64, // percentage
}

#[async_trait]
impl Query for GetShipmentPerformanceMetricsQuery {
    type Result = ShipmentPerformanceMetrics;

    async fn execute(&self, db_pool: Arc<DbPool>) -> Result<Self::Result, ServiceError> {
        let db = get_db(&db_pool).await?;

        // Calculate total shipments
        let total_shipments = Shipment::find()
            .filter(Shipment::Column::CreatedAt.between(self.start_date, self.end_date))
            .count(&db)
            .await
            .map_err(|e| {
                log::error!("Database error fetching total_shipments in GetShipmentPerformanceMetricsQuery: {:?}", e);
                ServiceError::DatabaseError
            })?;

        // Calculate on-time shipments
        let on_time_shipments = Shipment::find()
            .filter(Shipment::Column::CreatedAt.between(self.start_date, self.end_date))
            .filter(Shipment::Column::Status.eq(ShipmentStatus::Delivered))
            .filter(Shipment::Column::ActualDeliveryDate.lte(Shipment::Column::EstimatedDeliveryDate))
            .count(&db)
            .await
            .map_err(|e| {
                log::error!("Database error fetching on_time_shipments in GetShipmentPerformanceMetricsQuery: {:?}", e);
                ServiceError::DatabaseError
            })?;

        // Calculate average transit time in hours
        // Assuming ShipDate and ActualDeliveryDate are DateTime<Utc> fields
        let average_transit_time = Shipment::find()
            .filter(Shipment::Column::CreatedAt.between(self.start_date, self.end_date))
            .filter(Shipment::Column::Status.eq(ShipmentStatus::Delivered))
            .select_only()
            .column_as(
                Function::Avg(
                    Expr::cust("EXTRACT(EPOCH FROM (\"actual_delivery_date\" - \"ship_date\")) / 3600"),
                ),
                "average_transit_time",
            )
            .into_model::<(Option<f64>,)>()
            .one(&db)
            .await
            .map_err(|e| {
                log::error!("Database error calculating average_transit_time in GetShipmentPerformanceMetricsQuery: {:?}", e);
                ServiceError::DatabaseError
            })?
            .unwrap_or((None, ))
            .0
            .unwrap_or(0.0);

        // Calculate average delivery accuracy
        let average_delivery_accuracy = if total_shipments > 0 {
            (on_time_shipments as f64 / total_shipments as f64) * 100.0
        } else {
            0.0
        };

        Ok(ShipmentPerformanceMetrics {
            total_shipments,
            on_time_shipments,
            average_transit_time,
            average_delivery_accuracy,
        })
    }
}

/// Struct to get top-performing carriers within a date range.
#[derive(Debug, Serialize, Deserialize)]
pub struct GetTopCarrierPerformanceQuery {
    pub start_date: DateTime<Utc>,
    pub end_date: DateTime<Utc>,
    pub limit: u64,
}

/// Struct representing carrier performance metrics.
#[derive(Debug, Serialize)]
pub struct CarrierPerformance {
    pub carrier_id: i32,
    pub carrier_name: String,
    pub total_shipments: i64,
    pub on_time_deliveries: i64,
    pub average_transit_time: f64, // in hours
}

#[async_trait]
impl Query for GetTopCarrierPerformanceQuery {
    type Result = Vec<CarrierPerformance>;

    async fn execute(&self, db_pool: Arc<DbPool>) -> Result<Self::Result, ServiceError> {
        let db = get_db(&db_pool).await?;

        // Perform a join between Shipment and Carrier
        let performances = Shipment::find()
            .inner_join(Carrier)
            .filter(Shipment::Column::CreatedAt.between(self.start_date, self.end_date))
            .filter(Shipment::Column::Status.eq(ShipmentStatus::Delivered))
            .select_only()
            .column(Carrier::Column::Id)
            .column(Carrier::Column::Name)
            .column_as(Function::Count(Expr::col("*")), "total_shipments")
            .column_as(
                Function::Sum(
                    Expr::cust(
                        "CASE WHEN \"actual_delivery_date\" <= \"estimated_delivery_date\" THEN 1 ELSE 0 END",
                    ),
                ),
                "on_time_deliveries",
            )
            .column_as(
                Function::Avg(
                    Expr::cust("EXTRACT(EPOCH FROM (\"actual_delivery_date\" - \"ship_date\")) / 3600"),
                ),
                "average_transit_time",
            )
            .group_by(Carrier::Column::Id)
            .order_by_desc(Function::Count(Expr::col("*")))
            .limit(self.limit)
            .into_model::<(i32, String, i64, Option<i64>, Option<f64>)>()
            .all(&db)
            .await
            .map_err(|e| {
                log::error!("Database error in GetTopCarrierPerformanceQuery: {:?}", e);
                ServiceError::DatabaseError
            })?;

        // Map the results to CarrierPerformance structs
        let carrier_performances = performances
            .into_iter()
            .map(
                |(carrier_id, carrier_name, total_shipments, on_time_deliveries, average_transit_time)| {
                    CarrierPerformance {
                        carrier_id,
                        carrier_name,
                        total_shipments,
                        on_time_deliveries: on_time_deliveries.unwrap_or(0),
                        average_transit_time: average_transit_time.unwrap_or(0.0),
                    }
                },
            )
            .collect();

        Ok(carrier_performances)
    }
}
