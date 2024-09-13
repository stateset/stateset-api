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
pub struct GetManufactureOrderByIdQuery {
    pub order_id: i32,
}

#[async_trait]
impl Query for GetManufactureOrderByIdQuery {
    type Result = manufacture_order_entity::Model;

    async fn execute(&self, db_pool: Arc<DbPool>) -> Result<Self::Result, ServiceError> {
        let db = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;

        manufacture_order_entity::Entity::find_by_id(self.order_id)
            .one(&db)
            .await
            .map_err(|_| ServiceError::NotFound)?
            .ok_or(ServiceError::NotFound)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetManufactureOrdersByStatusQuery {
    pub status: ManufactureOrderStatus,
    pub limit: u64,
    pub offset: u64,
}

#[async_trait]
impl Query for GetManufactureOrdersByStatusQuery {
    type Result = Vec<manufacture_order_entity::Model>;

    async fn execute(&self, db_pool: Arc<DbPool>) -> Result<Self::Result, ServiceError> {
        let db = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;

        manufacture_order_entity::Entity::find()
            .filter(manufacture_order_entity::Column::Status.eq(self.status))
            .order_by_desc(manufacture_order_entity::Column::CreatedAt)
            .limit(self.limit)
            .offset(self.offset)
            .all(&db)
            .await
            .map_err(|_| ServiceError::DatabaseError)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetManufactureOrderDetailsQuery {
    pub order_id: i32,
}

#[derive(Debug, Serialize)]
pub struct ManufactureOrderDetails {
    pub order: manufacture_order_entity::Model,
    pub product: product_entity::Model,
    pub components: Vec<ManufactureOrderComponent>,
    pub operations: Vec<manufacture_order_operation_entity::Model>,
}

#[derive(Debug, Serialize)]
pub struct ManufactureOrderComponent {
    pub component_id: i32,
    pub component_name: String,
    pub required_quantity: f64,
    pub allocated_quantity: f64,
}

#[async_trait]
impl Query for GetManufactureOrderDetailsQuery {
    type Result = ManufactureOrderDetails;

    async fn execute(&self, db_pool: Arc<DbPool>) -> Result<Self::Result, ServiceError> {
        let db = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;

        let order = manufacture_order_entity::Entity::find_by_id(self.order_id)
            .one(&db)
            .await
            .map_err(|_| ServiceError::NotFound)?
            .ok_or(ServiceError::NotFound)?;

        let product = product_entity::Entity::find_by_id(order.product_id)
            .one(&db)
            .await
            .map_err(|_| ServiceError::NotFound)?
            .ok_or(ServiceError::NotFound)?;

        let components = manufacture_order_component_entity::Entity::find()
            .filter(manufacture_order_component_entity::Column::ManufactureOrderId.eq(self.order_id))
            .find_also_related(component_entity::Entity)
            .all(&db)
            .await
            .map_err(|_| ServiceError::DatabaseError)?;

        let operations = manufacture_order_operation_entity::Entity::find()
            .filter(manufacture_order_operation_entity::Column::ManufactureOrderId.eq(self.order_id))
            .all(&db)
            .await
            .map_err(|_| ServiceError::DatabaseError)?;

        let manufacture_components = components
            .into_iter()
            .map(|(component, component_data)| ManufactureOrderComponent {
                component_id: component_data.unwrap().id,
                component_name: component_data.unwrap().name,
                required_quantity: component.required_quantity,
                allocated_quantity: component.allocated_quantity,
            })
            .collect();

        Ok(ManufactureOrderDetails {
            order,
            product,
            components: manufacture_components,
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
        let db = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;

        let total_orders = manufacture_order_entity::Entity::find()
            .filter(manufacture_order_entity::Column::CreatedAt.between(self.start_date, self.end_date))
            .count(&db)
            .await
            .map_err(|_| ServiceError::DatabaseError)?;

        let completed_orders = manufacture_order_entity::Entity::find()
            .filter(manufacture_order_entity::Column::CreatedAt.between(self.start_date, self.end_date))
            .filter(manufacture_order_entity::Column::Status.eq(ManufactureOrderStatus::Completed))
            .count(&db)
            .await
            .map_err(|_| ServiceError::DatabaseError)?;

        let on_time_completions = manufacture_order_entity::Entity::find()
            .filter(manufacture_order_entity::Column::CreatedAt.between(self.start_date, self.end_date))
            .filter(manufacture_order_entity::Column::Status.eq(ManufactureOrderStatus::Completed))
            .filter(manufacture_order_entity::Column::ActualEndDate.lte(manufacture_order_entity::Column::PlannedEndDate))
            .count(&db)
            .await
            .map_err(|_| ServiceError::DatabaseError)?;

        let on_time_completion_rate = if completed_orders > 0 {
            on_time_completions as f64 / completed_orders as f64
        } else {
            0.0
        };

        let average_cycle_time = manufacture_order_entity::Entity::find()
            .filter(manufacture_order_entity::Column::CreatedAt.between(self.start_date, self.end_date))
            .filter(manufacture_order_entity::Column::Status.eq(ManufactureOrderStatus::Completed))
            .select_only()
            .column_as(avg(manufacture_order_entity::Column::ActualEndDate - manufacture_order_entity::Column::ActualStartDate), "average_cycle_time")
            .into_tuple()
            .one(&db)
            .await
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
        let db = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;

        let utilizations = manufacture_order_operation_entity::Entity::find()
            .filter(manufacture_order_operation_entity::Column::StartTime.between(self.start_date, self.end_date))
            .group_by(manufacture_order_operation_entity::Column::ResourceId)
            .select_only()
            .column(manufacture_order_operation_entity::Column::ResourceId)
            .column_as(sum(manufacture_order_operation_entity::Column::EndTime - manufacture_order_operation_entity::Column::StartTime), "utilized_hours")
            .all(&db)
            .await
            .map_err(|_| ServiceError::DatabaseError)?;

        let total_hours = (self.end_date - self.start_date).num_hours() as f64;

        Ok(utilizations
            .into_iter()
            .map(|(resource_id, utilized_hours)| ResourceUtilization {
                resource_id,
                resource_name: resource_id.to_string(), // Assuming resource name needs to be fetched separately if needed
                total_hours,
                utilized_hours: utilized_hours.unwrap_or(0.0),
                utilization_rate: utilized_hours.unwrap_or(0.0) / total_hours,
            })
            .collect())
    }
}
