use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sea_orm::{
    prelude::*, query::*, DatabaseConnection, EntityTrait, RelationTrait,
    sea_query::{Func, Expr, Alias}, IntoSimpleExpr,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::{
    db::DbPool, errors::ServiceError,
    models::{
        shipment::{Entity as ShipmentEntity, Model as ShipmentModel, ShipmentStatus},
        shipment_item::{Entity as ShipmentItemEntity, Model as ShipmentItemModel},
        shipment_event::{self, Entity as TrackingEventEntity},
        order::{Entity as OrderEntity, Model as OrderModel},
        TrackingEvent,
    },
};
// Models imported via wildcard above

/// Trait representing a generic asynchronous query.
#[async_trait]
pub trait Query: Send + Sync {
    type Result: Send + Sync;

    /// Executes the query using the provided database pool.
    async fn execute(&self, db_pool: &DatabaseConnection) -> Result<Self::Result, ServiceError>;
}

// Note: DbPool is already a DatabaseConnection, so we can use it directly

/// Struct to get a specific shipment by ID.
#[derive(Debug, Serialize, Deserialize)]
pub struct GetShipmentByIdQuery {
    pub shipment_id: i32,
}

#[async_trait]
impl Query for GetShipmentByIdQuery {
    type Result = ShipmentModel;

    async fn execute(&self, db_pool: &DatabaseConnection) -> Result<Self::Result, ServiceError> {
        
        ShipmentEntity::find_by_id(self.shipment_id)
            .one(db_pool)
            .await
            .map_err(|e| {
                log::error!("Database error in GetShipmentByIdQuery: {:?}", e);
            })?
            .ok_or(ServiceError::NotFound("Not found".to_string()))
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
    type Result = Vec<ShipmentModel>;

    async fn execute(&self, db_pool: &DatabaseConnection) -> Result<Self::Result, ServiceError> {
        
        ShipmentEntity::find()
            .filter(ShipmentEntity::Column::Status.eq(self.status))
            .order_by_desc(ShipmentEntity::Column::CreatedAt)
            .limit(self.limit)
            .offset(self.offset)
            .all(db_pool)
            .await
            .map_err(|e| {
                log::error!("Database error in GetShipmentsByStatusQuery: {:?}", e);
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
    type Result = Vec<ShipmentModel>;

    async fn execute(&self, db_pool: &DatabaseConnection) -> Result<Self::Result, ServiceError> {
        
        ShipmentEntity::find()
            .filter(crate::models::shipment::Column::CreatedAt.between(self.start_date, self.end_date))
            .order_by_desc(crate::models::shipment::Column::CreatedAt)
            .limit(self.limit)
            .offset(self.offset)
            .all(db_pool)
            .await
            .map_err(|e| {
                log::error!("Database error in GetShipmentsInDateRangeQuery: {:?}", e);
                ServiceError::db_error(e)
            })
    }
}

/// Struct to get detailed information about a specific shipment.
#[derive(Debug, Serialize, Deserialize)]
pub struct GetShipmentDetailsQuery {
    pub shipment_id: uuid::Uuid,
}

/// Struct representing detailed information of a shipment.
#[derive(Debug, Serialize)]
pub struct ShipmentDetails {
    pub shipment: ShipmentModel,
    pub order: OrderModel,
    pub items: Vec<ShipmentItemModel>,
    pub tracking_events: Vec<TrackingEvent>,
}

#[async_trait]
impl Query for GetShipmentDetailsQuery {
    type Result = ShipmentDetails;

    async fn execute(&self, db_pool: &DatabaseConnection) -> Result<Self::Result, ServiceError> {
        
        // Fetch the shipment
        let shipment = ShipmentEntity::find_by_id(self.shipment_id)
            .one(db_pool)
            .await
            .map_err(|e| {
                log::error!(
                    "Database error fetching shipment in GetShipmentDetailsQuery: {:?}",
                    e
                );
                ServiceError::db_error(e)
            })?
            .ok_or_else(|| ServiceError::NotFound("Shipment not found".to_string()))?;

        // Fetch the associated order
        let order = OrderEntity::find_by_id(shipment.order_id)
            .one(db_pool)
            .await
            .map_err(|e| {
                log::error!(
                    "Database error fetching order in GetShipmentDetailsQuery: {:?}",
                    e
                );
            })?
            .ok_or(ServiceError::NotFound)?;

        // Fetch shipment items
        let items = ShipmentItemEntity::find()
            .filter(crate::models::shipment_item::Column::ShipmentId.eq(self.shipment_id))
            .all(db_pool)
            .await
            .map_err(|e| {
                log::error!(
                    "Database error fetching shipment items in GetShipmentDetailsQuery: {:?}",
                    e
                );
                ServiceError::db_error(e)
            })?;

        // Get tracking events if needed
        let tracking_events = TrackingEventEntity::find()
            .filter(shipment_event::Column::ShipmentId.eq(self.shipment_id))
            .order_by_desc(shipment_event::Column::Timestamp)
            .all(db_pool)
            .await
            .map_err(|e| ServiceError::db_error(e))?;

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

    async fn execute(&self, db_pool: &DatabaseConnection) -> Result<Self::Result, ServiceError> {
        
        // Calculate total shipments
        let total_shipments = ShipmentEntity::find()
            .filter(crate::models::shipment::Column::CreatedAt.between(self.start_date, self.end_date))
            .count(db_pool)
            .await
            .map_err(|e| ServiceError::db_error(e))?;

        // Calculate on-time shipments
        let on_time_shipments = ShipmentEntity::find()
            .filter(crate::models::shipment::Column::CreatedAt.between(self.start_date, self.end_date))
            .filter(crate::models::shipment::Column::Status.eq(ShipmentStatus::Delivered))
            .filter(crate::models::shipment::Column::DeliveredAt.lte(crate::models::shipment::Column::EstimatedDelivery))
            .count(db_pool)
            .await
            .map_err(|e| ServiceError::db_error(e))?;

        let on_time_delivery_rate = if total_shipments > 0 {
            on_time_shipments as f64 / total_shipments as f64
        } else {
            0.0
        };

        // Calculate average transit time in hours
        // Assuming ShipDate and ActualDeliveryDate are DateTime<Utc> fields
        let average_transit_time = ShipmentEntity::find()
            .filter(crate::models::shipment::Column::CreatedAt.between(self.start_date, self.end_date))
            .filter(crate::models::shipment::Column::Status.eq(ShipmentStatus::Delivered))
            .select_only()
            .column_as(
                Expr::cust("AVG(EXTRACT(EPOCH FROM (actual_delivery_date - ship_date)) / 3600)"),
                "average_transit_time",
            )
            .into_tuple::<Option<f64>>()
            .one(db_pool)
            .await
            .map_err(|e| ServiceError::db_error(e))?
            .flatten()
            .unwrap_or(0.0);

        // Calculate average delivery accuracy
        let average_delivery_accuracy = if total_shipments > 0 {
            (on_time_shipments as f64 / total_shipments as f64) * 100.0
        } else {
            0.0
        };

        Ok(ShipmentPerformanceMetrics {
            total_shipments: total_shipments.try_into().unwrap_or(0),
            on_time_shipments: on_time_shipments.try_into().unwrap_or(0),
            average_transit_time,
            average_delivery_accuracy,
        })
    }
}

/* TODO: Implement GetTopCarrierPerformanceQuery once Carrier entity is created

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

    async fn execute(&self, db_pool: &DatabaseConnection) -> Result<Self::Result, ServiceError> {
        
        // Perform a join between Shipment and Carrier
        let performances = ShipmentEntity::find()
            .inner_join(Carrier)
            .filter(ShipmentEntity::Column::CreatedAt.between(self.start_date, self.end_date))
            .filter(ShipmentEntity::Column::Status.eq(ShipmentStatus::Delivered))
            .select_only()
            .column(Carrier::Column::Id)
            .column(Carrier::Column::Name)
            .column_as(Func::Count(Expr::col("*")), "total_shipments")
            .column_as(
                Func::Sum(
                    Expr::cust(
                        "CASE WHEN \"actual_delivery_date\" <= \"estimated_delivery_date\" THEN 1 ELSE 0 END",
                    ),
                ),
                "on_time_deliveries",
            )
            .column_as(
                Func::Avg(
                    Expr::cust("EXTRACT(EPOCH FROM (\"actual_delivery_date\" - \"ship_date\")) / 3600"),
                ),
                "average_transit_time",
            )
            .group_by(Carrier::Column::Id)
            .order_by_desc(Func::Count(Expr::col("*")))
            .limit(self.limit)
            .into_model::<(i32, String, i64, Option<i64>, Option<f64>)>()
            .all(db_pool)
            .await
            .map_err(|e| {
                log::error!("Database error in GetTopCarrierPerformanceQuery: {:?}", e);
            })?;

        // Map the results to CarrierPerformance structs
        let carrier_performances = performances
            .into_iter()
            .map(
                |(
                    carrier_id,
                    carrier_name,
                    total_shipments,
                    on_time_deliveries,
                    average_transit_time,
                )| {
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
*/
