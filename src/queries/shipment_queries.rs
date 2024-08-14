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
pub struct GetShipmentByIdQuery {
    pub shipment_id: i32,
}

#[async_trait]
impl Query for GetShipmentByIdQuery {
    type Result = Shipment;

    async fn execute(&self, db_pool: Arc<DbPool>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;
        let shipment = shipments::table
            .find(self.shipment_id)
            .first::<Shipment>(&conn)
            .map_err(|_| ServiceError::NotFound)?;
        Ok(shipment)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetShipmentsByStatusQuery {
    pub status: ShipmentStatus,
    pub limit: i64,
    pub offset: i64,
}

#[async_trait]
impl Query for GetShipmentsByStatusQuery {
    type Result = Vec<Shipment>;

    async fn execute(&self, db_pool: Arc<DbPool>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;
        let shipments = shipments::table
            .filter(shipments::status.eq(self.status))
            .order(shipments::created_at.desc())
            .limit(self.limit)
            .offset(self.offset)
            .load::<Shipment>(&conn)
            .map_err(|_| ServiceError::DatabaseError)?;
        Ok(shipments)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetShipmentsInDateRangeQuery {
    pub start_date: DateTime<Utc>,
    pub end_date: DateTime<Utc>,
    pub limit: i64,
    pub offset: i64,
}

#[async_trait]
impl Query for GetShipmentsInDateRangeQuery {
    type Result = Vec<Shipment>;

    async fn execute(&self, db_pool: Arc<DbPool>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;
        let shipments = shipments::table
            .filter(shipments::created_at.between(self.start_date, self.end_date))
            .order(shipments::created_at.desc())
            .limit(self.limit)
            .offset(self.offset)
            .load::<Shipment>(&conn)
            .map_err(|_| ServiceError::DatabaseError)?;
        Ok(shipments)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetShipmentDetailsQuery {
    pub shipment_id: i32,
}

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
        let conn = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;
        
        let shipment = shipments::table
            .find(self.shipment_id)
            .first::<Shipment>(&conn)
            .map_err(|_| ServiceError::NotFound)?;

        let order = orders::table
            .find(shipment.order_id)
            .first::<Order>(&conn)
            .map_err(|_| ServiceError::NotFound)?;

        let items = ShipmentItem::belonging_to(&shipment)
            .load::<ShipmentItem>(&conn)
            .map_err(|_| ServiceError::DatabaseError)?;

        let tracking_events = TrackingEvent::belonging_to(&shipment)
            .order(tracking_events::timestamp.desc())
            .load::<TrackingEvent>(&conn)
            .map_err(|_| ServiceError::DatabaseError)?;

        Ok(ShipmentDetails {
            shipment,
            order,
            items,
            tracking_events,
        })
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetShipmentPerformanceMetricsQuery {
    pub start_date: DateTime<Utc>,
    pub end_date: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct ShipmentPerformanceMetrics {
    pub total_shipments: i64,
    pub on_time_shipments: i64,
    pub average_transit_time: f64, // in hours
    pub average_delivery_accuracy: f64, // percentage
}

#[async_trait]
impl Query for GetShipmentPerformanceMetricsQuery {
    type Result = ShipmentPerformanceMetrics;

    async fn execute(&self, db_pool: Arc<DbPool>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;
        
        let total_shipments: i64 = shipments::table
            .filter(shipments::created_at.between(self.start_date, self.end_date))
            .count()
            .get_result(&conn)
            .map_err(|_| ServiceError::DatabaseError)?;

        let on_time_shipments: i64 = shipments::table
            .filter(shipments::created_at.between(self.start_date, self.end_date))
            .filter(shipments::status.eq(ShipmentStatus::Delivered))
            .filter(shipments::actual_delivery_date.le(shipments::estimated_delivery_date))
            .count()
            .get_result(&conn)
            .map_err(|_| ServiceError::DatabaseError)?;

        let average_transit_time: f64 = shipments::table
            .filter(shipments::created_at.between(self.start_date, self.end_date))
            .filter(shipments::status.eq(ShipmentStatus::Delivered))
            .select(avg(shipments::actual_delivery_date - shipments::ship_date))
            .first::<Option<f64>>(&conn)
            .map_err(|_| ServiceError::DatabaseError)?
            .unwrap_or(0.0);

        let average_delivery_accuracy: f64 = if total_shipments > 0 {
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

#[derive(Debug, Serialize, Deserialize)]
pub struct GetTopCarrierPerformanceQuery {
    pub start_date: DateTime<Utc>,
    pub end_date: DateTime<Utc>,
    pub limit: i64,
}

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
        let conn = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;
        
        let performances = shipments::table
            .inner_join(carriers::table)
            .filter(shipments::created_at.between(self.start_date, self.end_date))
            .filter(shipments::status.eq(ShipmentStatus::Delivered))
            .group_by((carriers::id, carriers::name))
            .order(count_star().desc())
            .limit(self.limit)
            .select((
                carriers::id,
                carriers::name,
                count_star(),
                sum(case_when(shipments::actual_delivery_date.le(shipments::estimated_delivery_date), 1).otherwise(0)),
                avg(shipments::actual_delivery_date - shipments::ship_date),
            ))
            .load::<(i32, String, i64, Option<i64>, Option<f64>)>(&conn)
            .map_err(|_| ServiceError::DatabaseError)?;

        Ok(performances
            .into_iter()
            .map(|(carrier_id, carrier_name, total_shipments, on_time_deliveries, avg_transit_time)| {
                CarrierPerformance {
                    carrier_id,
                    carrier_name,
                    total_shipments,
                    on_time_deliveries: on_time_deliveries.unwrap_or(0),
                    average_transit_time: avg_transit_time.unwrap_or(0.0),
                }
            })
            .collect())
    }
}