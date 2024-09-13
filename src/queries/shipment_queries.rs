use async_trait::async_trait;
use serde::{Serialize, Deserialize};
use std::sync::Arc;
use sea_orm::{
    QuerySelect,
    QueryOrder,
    QueryFilter,
    EntityTrait,
    RelationTrait,
    query::*,
    Expr,
    Function::*,
};
use crate::{errors::ServiceError, db::DbPool, models::*};
use chrono::{DateTime, Utc};

use crate::billofmaterials::BillOfMaterials;
use crate::inventory_item::InventoryItem;
use crate::order::Order;
use crate::shipment::Shipment;
use crate::shipment_item::ShipmentItem;
use crate::carrier::Carrier;
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
pub struct GetShipmentByIdQuery {
    pub shipment_id: i32,
}

#[async_trait]
impl Query for GetShipmentByIdQuery {
    type Result = Shipment;

    async fn execute(&self, db_pool: Arc<DbPool>) -> Result<Self::Result, ServiceError> {
        let db = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;

        Shipment::find_by_id(self.shipment_id)
            .one(&db)
            .await
            .map_err(|_| ServiceError::NotFound)?
            .ok_or(ServiceError::NotFound)
    }
}

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
        let db = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;

        Shipment::find()
            .filter(Shipment::Column::Status.eq(self.status))
            .order_by_desc(Shipment::Column::CreatedAt)
            .limit(self.limit)
            .offset(self.offset)
            .all(&db)
            .await
            .map_err(|_| ServiceError::DatabaseError)
    }
}

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
        let db = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;

        Shipment::find()
            .filter(Shipment::Column::CreatedAt.between(self.start_date, self.end_date))
            .order_by_desc(Shipment::Column::CreatedAt)
            .limit(self.limit)
            .offset(self.offset)
            .all(&db)
            .await
            .map_err(|_| ServiceError::DatabaseError)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetShipmentDetailsQuery {
    pub shipment_id: i32,
}

#[derive(Debug, Serialize)]
pub struct ShipmentDetails {
    pub shipment: shipment_entity::Model,
    pub order: Order,
    pub items: Vec<OrderItem>,
    pub tracking_events: Vec<TrackingEvent>,
}

#[async_trait]
impl Query for GetShipmentDetailsQuery {
    type Result = ShipmentDetails;

    async fn execute(&self, db_pool: Arc<DbPool>) -> Result<Self::Result, ServiceError> {
        let db = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;

        let shipment = Shipment::find_by_id(self.shipment_id)
            .one(&db)
            .await
            .map_err(|_| ServiceError::NotFound)?
            .ok_or(ServiceError::NotFound)?;

        let order = Order::find_by_id(shipment.order_id)
            .one(&db)
            .await
            .map_err(|_| ServiceError::NotFound)?
            .ok_or(ServiceError::NotFound)?;

        let items = ShipmentItem::find()
            .filter(ShipmentItem::Column::ShipmentId.eq(self.shipment_id))
            .all(&db)
            .await
            .map_err(|_| ServiceError::DatabaseError)?;

        let tracking_events = TrackingEvent::find()
            .filter(TrackingEvent::Column::ShipmentId.eq(self.shipment_id))
            .order_by_desc(TrackingEvent::Column::Timestamp)
            .all(&db)
            .await
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
        let db = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;

        let total_shipments = Shipment::find()
            .filter(Shipment::Column::CreatedAt.between(self.start_date, self.end_date))
            .count(&db)
            .await
            .map_err(|_| ServiceError::DatabaseError)?;

        let on_time_shipments = Shipment::find()
            .filter(Shipment::Column::CreatedAt.between(self.start_date, self.end_date))
            .filter(Shipment::Column::Status.eq(ShipmentStatus::Delivered))
            .filter(Shipment::Column::ActualDeliveryDate.lte(Shipment::Column::EstimatedDeliveryDate))
            .count(&db)
            .await
            .map_err(|_| ServiceError::DatabaseError)?;

        let average_transit_time = Shipment::find()
            .filter(Shipment::Column::CreatedAt.between(self.start_date, self.end_date))
            .filter(Shipment::Column::Status.eq(ShipmentStatus::Delivered))
            .select_only()
            .column_as(avg(Shipment::Column::ActualDeliveryDate - Shipment::Column::ShipDate), "average_transit_time")
            .into_tuple()
            .one(&db)
            .await
            .map_err(|_| ServiceError::DatabaseError)?
            .unwrap_or(0.0);

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

#[derive(Debug, Serialize, Deserialize)]
pub struct GetTopCarrierPerformanceQuery {
    pub start_date: DateTime<Utc>,
    pub end_date: DateTime<Utc>,
    pub limit: u64,
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
        let db = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;

        let performances = Shipment::find()
            .inner_join(Carrier)
            .filter(Shipment::Column::CreatedAt.between(self.start_date, self.end_date))
            .filter(Shipment::Column::Status.eq(ShipmentStatus::Delivered))
            .group_by(Carrier::Column::Id)
            .order_by_desc(count_star())
            .select_only()
            .column(Carrier::Column::Id)
            .column(Carrier::Column::Name)
            .column_as(count_star(), "total_shipments")
            .column_as(sum(case_when(Shipment::Column::ActualDeliveryDate.lte(Shipment::Column::EstimatedDeliveryDate), 1).otherwise(0)), "on_time_deliveries")
            .column_as(avg(Shipment::Column::ActualDeliveryDate - Shipment::Column::ShipDate), "average_transit_time")
            .into_tuple()
            .all(&db)
            .await
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
