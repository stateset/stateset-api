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
pub struct GetManufactureOrderByIdQuery {
    pub order_id: i32,
}

#[async_trait]
impl Query for GetManufactureOrderByIdQuery {
    type Result = ManufactureOrder;

    async fn execute(&self, db_pool: Arc<DbPool>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;
        let order = manufacture_orders::table
            .find(self.order_id)
            .first::<ManufactureOrder>(&conn)
            .map_err(|_| ServiceError::NotFound)?;
        Ok(order)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetManufactureOrdersByStatusQuery {
    pub status: ManufactureOrderStatus,
    pub limit: i64,
    pub offset: i64,
}

#[async_trait]
impl Query for GetManufactureOrdersByStatusQuery {
    type Result = Vec<ManufactureOrder>;

    async fn execute(&self, db_pool: Arc<DbPool>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;
        let orders = manufacture_orders::table
            .filter(manufacture_orders::status.eq(self.status))
            .order(manufacture_orders::created_at.desc())
            .limit(self.limit)
            .offset(self.offset)
            .load::<ManufactureOrder>(&conn)
            .map_err(|_| ServiceError::DatabaseError)?;
        Ok(orders)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetManufactureOrderDetailsQuery {
    pub order_id: i32,
}

#[derive(Debug, Serialize)]
pub struct ManufactureOrderDetails {
    pub order: ManufactureOrder,
    pub product: Product,
    pub components: Vec<ManufactureOrderComponent>,
    pub operations: Vec<ManufactureOrderOperation>,
}

#[derive(Debug, Serialize)]
pub struct ManufactureOrderComponent {
    pub component_id: i32,
    pub component_name: String,
    pub required_quantity: f64,
    pub allocated_quantity: f64,
}

#[derive(Debug, Serialize)]
pub struct ManufactureOrderOperation {
    pub operation_id: i32,
    pub operation_name: String,
    pub status: OperationStatus,
    pub start_time: Option<DateTime<Utc>>,
    pub end_time: Option<DateTime<Utc>>,
}

#[async_trait]
impl Query for GetManufactureOrderDetailsQuery {
    type Result = ManufactureOrderDetails;

    async fn execute(&self, db_pool: Arc<DbPool>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;
        
        let order = manufacture_orders::table
            .find(self.order_id)
            .first::<ManufactureOrder>(&conn)
            .map_err(|_| ServiceError::NotFound)?;

        let product = products::table
            .find(order.product_id)
            .first::<Product>(&conn)
            .map_err(|_| ServiceError::NotFound)?;

        let components = manufacture_order_components::table
            .inner_join(components::table)
            .filter(manufacture_order_components::manufacture_order_id.eq(self.order_id))
            .select((
                components::id,
                components::name,
                manufacture_order_components::required_quantity,
                manufacture_order_components::allocated_quantity,
            ))
            .load::<(i32, String, f64, f64)>(&conn)
            .map_err(|_| ServiceError::DatabaseError)?
            .into_iter()
            .map(|(id, name, required, allocated)| ManufactureOrderComponent {
                component_id: id,
                component_name: name,
                required_quantity: required,
                allocated_quantity: allocated,
            })
            .collect();

        let operations = manufacture_order_operations::table
            .filter(manufacture_order_operations::manufacture_order_id.eq(self.order_id))
            .load::<ManufactureOrderOperation>(&conn)
            .map_err(|_| ServiceError::DatabaseError)?;

        Ok(ManufactureOrderDetails {
            order,
            product,
            components,
            operations,
        })
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetManufactureOrderEfficiencyQuery {
    pub start_date: DateTime<Utc>,
    pub end_date: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct ManufactureOrderEfficiency {
    pub total_orders: i64,
    pub completed_orders: i64,
    pub on_time_completion_rate: f64,
    pub average_cycle_time: f64, // in hours
}

#[async_trait]
impl Query for GetManufactureOrderEfficiencyQuery {
    type Result = ManufactureOrderEfficiency;

    async fn execute(&self, db_pool: Arc<DbPool>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;
        
        let total_orders: i64 = manufacture_orders::table
            .filter(manufacture_orders::created_at.between(self.start_date, self.end_date))
            .count()
            .get_result(&conn)
            .map_err(|_| ServiceError::DatabaseError)?;

        let completed_orders: i64 = manufacture_orders::table
            .filter(manufacture_orders::created_at.between(self.start_date, self.end_date))
            .filter(manufacture_orders::status.eq(ManufactureOrderStatus::Completed))
            .count()
            .get_result(&conn)
            .map_err(|_| ServiceError::DatabaseError)?;

        let on_time_completions: i64 = manufacture_orders::table
            .filter(manufacture_orders::created_at.between(self.start_date, self.end_date))
            .filter(manufacture_orders::status.eq(ManufactureOrderStatus::Completed))
            .filter(manufacture_orders::actual_end_date.le(manufacture_orders::planned_end_date))
            .count()
            .get_result(&conn)
            .map_err(|_| ServiceError::DatabaseError)?;

        let on_time_completion_rate = if completed_orders > 0 {
            on_time_completions as f64 / completed_orders as f64
        } else {
            0.0
        };

        let average_cycle_time: f64 = manufacture_orders::table
            .filter(manufacture_orders::created_at.between(self.start_date, self.end_date))
            .filter(manufacture_orders::status.eq(ManufactureOrderStatus::Completed))
            .select(avg(manufacture_orders::actual_end_date - manufacture_orders::actual_start_date))
            .first::<Option<f64>>(&conn)
            .map_err(|_| ServiceError::DatabaseError)?
            .unwrap_or(0.0);

        Ok(ManufactureOrderEfficiency {
            total_orders,
            completed_orders,
            on_time_completion_rate,
            average_cycle_time,
        })
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetResourceUtilizationQuery {
    pub start_date: DateTime<Utc>,
    pub end_date: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct ResourceUtilization {
    pub resource_id: i32,
    pub resource_name: String,
    pub total_hours: f64,
    pub utilized_hours: f64,
    pub utilization_rate: f64,
}

#[async_trait]
impl Query for GetResourceUtilizationQuery {
    type Result = Vec<ResourceUtilization>;

    async fn execute(&self, db_pool: Arc<DbPool>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;
        
        let utilizations = manufacture_order_operations::table
            .inner_join(resources::table)
            .filter(manufacture_order_operations::start_time.between(self.start_date, self.end_date))
            .group_by((resources::id, resources::name))
            .select((
                resources::id,
                resources::name,
                sum(manufacture_order_operations::end_time - manufacture_order_operations::start_time),
            ))
            .load::<(i32, String, Option<f64>)>(&conn)
            .map_err(|_| ServiceError::DatabaseError)?;

        let total_hours = (self.end_date - self.start_date).num_hours() as f64;

        Ok(utilizations
            .into_iter()
            .map(|(id, name, utilized_hours)| {
                let utilized = utilized_hours.unwrap_or(0.0);
                ResourceUtilization {
                    resource_id: id,
                    resource_name: name,
                    total_hours,
                    utilized_hours: utilized,
                    utilization_rate: utilized / total_hours,
                }
            })
            .collect())
    }
}