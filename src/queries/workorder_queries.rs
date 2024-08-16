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
pub struct GetWorkOrderByIdQuery {
    pub work_order_id: i32,
}

#[async_trait]
impl Query for GetWorkOrderByIdQuery {
    type Result = WorkOrder::Model;

    async fn execute(&self, db_pool: Arc<DbPool>) -> Result<Self::Result, ServiceError> {
        let db = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;

        WorkOrder::find_by_id(self.work_order_id)
            .one(&db)
            .await
            .map_err(|_| ServiceError::NotFound)?
            .ok_or(ServiceError::NotFound)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetWorkOrdersByStatusQuery {
    pub status: WorkOrderStatus,
    pub limit: u64,
    pub offset: u64,
}

#[async_trait]
impl Query for GetWorkOrdersByStatusQuery {
    type Result = Vec<WorkOrder::Model>;

    async fn execute(&self, db_pool: Arc<DbPool>) -> Result<Self::Result, ServiceError> {
        let db = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;

        WorkOrder::find()
            .filter(WorkOrder::Column::Status.eq(self.status))
            .order_by_desc(WorkOrder::Column::CreatedAt)
            .limit(self.limit)
            .offset(self.offset)
            .all(&db)
            .await
            .map_err(|_| ServiceError::DatabaseError)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetWorkOrdersInDateRangeQuery {
    pub start_date: DateTime<Utc>,
    pub end_date: DateTime<Utc>,
    pub limit: u64,
    pub offset: u64,
}

#[async_trait]
impl Query for GetWorkOrdersInDateRangeQuery {
    type Result = Vec<WorkOrder::Model>;

    async fn execute(&self, db_pool: Arc<DbPool>) -> Result<Self::Result, ServiceError> {
        let db = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;

        WorkOrder::Entity::find()
            .filter(WorkOrder::Column::CreatedAt.between(self.start_date, self.end_date))
            .order_by_desc(WorkOrder::Column::CreatedAt)
            .limit(self.limit)
            .offset(self.offset)
            .all(&db)
            .await
            .map_err(|_| ServiceError::DatabaseError)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetWorkOrderDetailsQuery {
    pub work_order_id: i32,
}

#[derive(Debug, Serialize)]
pub struct WorkOrderDetails {
    pub work_order: WorkOrder::Model,
    pub tasks: Vec<work_order_task_entity::Model>,
    pub materials: Vec<work_order_material_entity::Model>,
}

#[async_trait]
impl Query for GetWorkOrderDetailsQuery {
    type Result = WorkOrderDetails;

    async fn execute(&self, db_pool: Arc<DbPool>) -> Result<Self::Result, ServiceError> {
        let db = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;

        let work_order = WorkOrder::Entity::find_by_id(self.work_order_id)
            .one(&db)
            .await
            .map_err(|_| ServiceError::NotFound)?
            .ok_or(ServiceError::NotFound)?;

        let tasks = work_order_task_entity::Entity::find()
            .filter(work_order_task_entity::Column::WorkOrderId.eq(self.work_order_id))
            .all(&db)
            .await
            .map_err(|_| ServiceError::DatabaseError)?;

        let materials = work_order_material_entity::Entity::find()
            .filter(work_order_material_entity::Column::WorkOrderId.eq(self.work_order_id))
            .all(&db)
            .await
            .map_err(|_| ServiceError::DatabaseError)?;

        Ok(WorkOrderDetails {
            work_order,
            tasks,
            materials,
        })
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetWorkOrderProductivityQuery {
    pub start_date: DateTime<Utc>,
    pub end_date: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct WorkOrderProductivity {
    pub total_work_orders: i64,
    pub completed_work_orders: i64,
    pub average_completion_time: f64, // in hours
}

#[async_trait]
impl Query for GetWorkOrderProductivityQuery {
    type Result = WorkOrderProductivity;

    async fn execute(&self, db_pool: Arc<DbPool>) -> Result<Self::Result, ServiceError> {
        let db = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;

        let total_work_orders = WorkOrder::Entity::find()
            .filter(WorkOrder::Column::CreatedAt.between(self.start_date, self.end_date))
            .count(&db)
            .await
            .map_err(|_| ServiceError::DatabaseError)?;

        let completed_work_orders = WorkOrder::Entity::find()
            .filter(WorkOrder::Column::CreatedAt.between(self.start_date, self.end_date))
            .filter(WorkOrder::Column::Status.eq(WorkOrderStatus::Completed))
            .count(&db)
            .await
            .map_err(|_| ServiceError::DatabaseError)?;

        let average_completion_time = WorkOrder::Entity::find()
            .filter(WorkOrder::Column::CreatedAt.between(self.start_date, self.end_date))
            .filter(WorkOrder::Column::Status.eq(WorkOrderStatus::Completed))
            .select_only()
            .column_as(avg(WorkOrder::Column::CompletionTime), "average_completion_time")
            .into_tuple()
            .one(&db)
            .await
            .map_err(|_| ServiceError::DatabaseError)?
            .unwrap_or(0.0);

        Ok(WorkOrderProductivity {
            total_work_orders,
            completed_work_orders,
            average_completion_time,
        })
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetTopPerformingTechniciansQuery {
    pub start_date: DateTime<Utc>,
    pub end_date: DateTime<Utc>,
    pub limit: u64,
}

#[derive(Debug, Serialize)]
pub struct TechnicianPerformance {
    pub technician_id: i32,
    pub technician_name: String,
    pub completed_work_orders: i64,
    pub average_completion_time: f64, // in hours
}

#[async_trait]
impl Query for GetTopPerformingTechniciansQuery {
    type Result = Vec<TechnicianPerformance>;

    async fn execute(&self, db_pool: Arc<DbPool>) -> Result<Self::Result, ServiceError> {
        let db = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;

        let performances = WorkOrder::Entity::find()
            .inner_join(technician_entity::Entity)
            .filter(WorkOrder::Column::CreatedAt.between(self.start_date, self.end_date))
            .filter(WorkOrder::Column::Status.eq(WorkOrderStatus::Completed))
            .group_by((technician_entity::Column::Id, technician_entity::Column::Name))
            .order_by_desc(count_star())
            .limit(self.limit)
            .select_only()
            .column(technician_entity::Column::Id)
            .column(technician_entity::Column::Name)
            .column_as(count_star(), "completed_work_orders")
            .column_as(avg(WorkOrder::Column::CompletionTime), "average_completion_time")
            .into_tuple()
            .all(&db)
            .await
            .map_err(|_| ServiceError::DatabaseError)?;

        Ok(performances
            .into_iter()
            .map(|(technician_id, technician_name, completed_work_orders, avg_completion_time)| {
                TechnicianPerformance {
                    technician_id,
                    technician_name,
                    completed_work_orders,
                    average_completion_time: avg_completion_time.unwrap_or(0.0),
                }
            })
            .collect())
    }
}
