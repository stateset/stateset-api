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
pub struct GetWorkOrderByIdQuery {
    pub work_order_id: i32,
}

#[async_trait]
impl Query for GetWorkOrderByIdQuery {
    type Result = WorkOrder;

    async fn execute(&self, db_pool: Arc<DbPool>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;
        let work_order = work_orders::table
            .find(self.work_order_id)
            .first::<WorkOrder>(&conn)
            .map_err(|_| ServiceError::NotFound)?;
        Ok(work_order)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetWorkOrdersByStatusQuery {
    pub status: WorkOrderStatus,
    pub limit: i64,
    pub offset: i64,
}

#[async_trait]
impl Query for GetWorkOrdersByStatusQuery {
    type Result = Vec<WorkOrder>;

    async fn execute(&self, db_pool: Arc<DbPool>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;
        let work_orders = work_orders::table
            .filter(work_orders::status.eq(self.status))
            .order(work_orders::created_at.desc())
            .limit(self.limit)
            .offset(self.offset)
            .load::<WorkOrder>(&conn)
            .map_err(|_| ServiceError::DatabaseError)?;
        Ok(work_orders)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetWorkOrdersInDateRangeQuery {
    pub start_date: DateTime<Utc>,
    pub end_date: DateTime<Utc>,
    pub limit: i64,
    pub offset: i64,
}

#[async_trait]
impl Query for GetWorkOrdersInDateRangeQuery {
    type Result = Vec<WorkOrder>;

    async fn execute(&self, db_pool: Arc<DbPool>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;
        let work_orders = work_orders::table
            .filter(work_orders::created_at.between(self.start_date, self.end_date))
            .order(work_orders::created_at.desc())
            .limit(self.limit)
            .offset(self.offset)
            .load::<WorkOrder>(&conn)
            .map_err(|_| ServiceError::DatabaseError)?;
        Ok(work_orders)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetWorkOrderDetailsQuery {
    pub work_order_id: i32,
}

#[derive(Debug, Serialize)]
pub struct WorkOrderDetails {
    pub work_order: WorkOrder,
    pub tasks: Vec<WorkOrderTask>,
    pub materials: Vec<WorkOrderMaterial>,
}

#[async_trait]
impl Query for GetWorkOrderDetailsQuery {
    type Result = WorkOrderDetails;

    async fn execute(&self, db_pool: Arc<DbPool>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;
        
        let work_order = work_orders::table
            .find(self.work_order_id)
            .first::<WorkOrder>(&conn)
            .map_err(|_| ServiceError::NotFound)?;

        let tasks = WorkOrderTask::belonging_to(&work_order)
            .load::<WorkOrderTask>(&conn)
            .map_err(|_| ServiceError::DatabaseError)?;

        let materials = WorkOrderMaterial::belonging_to(&work_order)
            .load::<WorkOrderMaterial>(&conn)
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
        let conn = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;
        
        let total_work_orders: i64 = work_orders::table
            .filter(work_orders::created_at.between(self.start_date, self.end_date))
            .count()
            .get_result(&conn)
            .map_err(|_| ServiceError::DatabaseError)?;

        let completed_work_orders: i64 = work_orders::table
            .filter(work_orders::created_at.between(self.start_date, self.end_date))
            .filter(work_orders::status.eq(WorkOrderStatus::Completed))
            .count()
            .get_result(&conn)
            .map_err(|_| ServiceError::DatabaseError)?;

        let average_completion_time: f64 = work_orders::table
            .filter(work_orders::created_at.between(self.start_date, self.end_date))
            .filter(work_orders::status.eq(WorkOrderStatus::Completed))
            .select(avg(work_orders::completion_time))
            .first::<Option<f64>>(&conn)
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
    pub limit: i64,
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
        let conn = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;
        
        let performances = work_orders::table
            .inner_join(technicians::table)
            .filter(work_orders::created_at.between(self.start_date, self.end_date))
            .filter(work_orders::status.eq(WorkOrderStatus::Completed))
            .group_by((technicians::id, technicians::name))
            .order(count_star().desc())
            .limit(self.limit)
            .select((
                technicians::id,
                technicians::name,
                count_star(),
                avg(work_orders::completion_time),
            ))
            .load::<(i32, String, i64, Option<f64>)>(&conn)
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