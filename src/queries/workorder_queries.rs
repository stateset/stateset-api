use crate::{db::DbPool, errors::ServiceError};
use crate::models::{
    work_order::{Entity as WorkOrderEntity, Model as WorkOrderModel, WorkOrderStatus},
    work_order_task_entity,
    work_order_material_entity,
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sea_orm::{
    prelude::*, query::*, EntityTrait, QueryFilter, QueryOrder, QuerySelect, RelationTrait,
    sea_query::{Func, Expr, Alias}, DatabaseConnection, IntoSimpleExpr,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

// Models imported via wildcard above

#[async_trait]
pub trait Query: Send + Sync {
    type Result: Send + Sync;

    async fn execute(&self, db_pool: &DatabaseConnection) -> Result<Self::Result, ServiceError>;
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetWorkOrderByIdQuery {
    pub work_order_id: Uuid,
}

#[async_trait]
impl Query for GetWorkOrderByIdQuery {
    type Result = WorkOrderModel;

    async fn execute(&self, db_pool: &DatabaseConnection) -> Result<Self::Result, ServiceError> {
        
        WorkOrderEntity::find_by_id(self.work_order_id)
            .one(db_pool)
            .await
            .map_err(|e| ServiceError::db_error(e))?
            .ok_or_else(|| ServiceError::NotFound("Work order not found".to_string()))
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
    type Result = Vec<WorkOrderModel>;

    async fn execute(&self, db_pool: &DatabaseConnection) -> Result<Self::Result, ServiceError> {
        
        WorkOrderEntity::find()
            .filter(crate::models::work_order::Column::Status.eq(self.status.clone()))
            .order_by_desc(crate::models::work_order::Column::CreatedAt)
            .limit(self.limit)
            .offset(self.offset)
            .all(db_pool)
            .await
            .map_err(|e| ServiceError::db_error(e))
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
    type Result = Vec<WorkOrderModel>;

    async fn execute(&self, db_pool: &DatabaseConnection) -> Result<Self::Result, ServiceError> {
        
        WorkOrderEntity::find()
            .filter(crate::models::work_order::Column::CreatedAt.between(self.start_date, self.end_date))
            .order_by_desc(crate::models::work_order::Column::CreatedAt)
            .limit(self.limit)
            .offset(self.offset)
            .all(db_pool)
            .await
            .map_err(|e| ServiceError::db_error(e))
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetWorkOrderDetailsQuery {
    pub work_order_id: Uuid,
}

#[derive(Debug, Serialize)]
pub struct WorkOrderDetails {
    pub work_order: WorkOrderModel,
    pub tasks: Vec<work_order_task_entity::Model>,
    pub materials: Vec<work_order_material_entity::Model>,
}

#[async_trait]
impl Query for GetWorkOrderDetailsQuery {
    type Result = WorkOrderDetails;

    async fn execute(&self, db_pool: &DatabaseConnection) -> Result<Self::Result, ServiceError> {
        
        let work_order = WorkOrderEntity::find_by_id(self.work_order_id)
            .one(db_pool)
            .await
            .map_err(|e| ServiceError::NotFound(e.to_string()))?
            .ok_or(ServiceError::NotFound("Work order not found".to_string()))?;

        let tasks = work_order_task_entity::Entity::find()
            .filter(work_order_task_entity::Column::WorkOrderId.eq(self.work_order_id))
            .all(db_pool)
            .await
            .map_err(|e| ServiceError::db_error(e))?;

        let materials = work_order_material_entity::Entity::find()
            .filter(work_order_material_entity::Column::WorkOrderId.eq(self.work_order_id))
            .all(db_pool)
            .await
            .map_err(|e| ServiceError::db_error(e))?;

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

    async fn execute(&self, db_pool: &DatabaseConnection) -> Result<Self::Result, ServiceError> {
        
        let total_work_orders = WorkOrderEntity::find()
            .filter(crate::models::work_order::Column::CreatedAt.between(self.start_date, self.end_date))
            .count(db_pool)
            .await
            .map_err(|e| ServiceError::db_error(e))?;

        let completed_work_orders = WorkOrderEntity::find()
            .filter(crate::models::work_order::Column::CreatedAt.between(self.start_date, self.end_date))
            .filter(crate::models::work_order::Column::Status.eq(WorkOrderStatus::Completed))
            .count(db_pool)
            .await
            .map_err(|e| ServiceError::db_error(e))?;

        // For now, let's simplify this - we can't directly average DateTime fields
        // We would need to calculate duration from created_at to completion_time
        let average_completion_time = 0.0; // Placeholder for now

        Ok(WorkOrderProductivity {
            total_work_orders: total_work_orders.try_into().unwrap(),
            completed_work_orders: completed_work_orders.try_into().unwrap(),
            average_completion_time,
        })
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetTopTechniciansPerformanceQuery {
    pub start_date: DateTime<Utc>,
    pub end_date: DateTime<Utc>,
    pub limit: u64,
}

#[derive(Debug, Serialize)]
pub struct TechnicianPerformance {
    pub technician_id: Uuid,
    pub technician_name: String,
    pub completed_work_orders: u64,
    pub average_completion_time: Option<f32>,
}

#[async_trait]
impl Query for GetTopTechniciansPerformanceQuery {
    type Result = Vec<TechnicianPerformance>;

    async fn execute(&self, _db_pool: &DatabaseConnection) -> Result<Self::Result, ServiceError> {
        // Technician performance tracking requires technician entity
        // Returns empty until technician assignment tracking is implemented
        Ok(vec![])
    }
}
