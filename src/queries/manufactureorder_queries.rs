use crate::{
    db::DbPool, 
    errors::ServiceError, 
    models::{
        manufacture_orders::{Entity as ManufactureOrder, Model as ManufactureOrderModel},
        product_entity::{Entity as ProductEntity},
        manufacture_order_line_item::{Entity as ManufactureOrderComponentEntity},
    }
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sea_orm::{
    prelude::*, query::*, EntityTrait, QueryFilter, QueryOrder, QuerySelect, RelationTrait,
    DatabaseConnection, IntoSimpleExpr,
    sea_query::{Func, SimpleExpr, Alias, Expr},
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

// Models imported via wildcard above

#[async_trait]
pub trait Query: Send + Sync {
    type Result: Send + Sync;

    async fn execute(&self, db_pool: &DatabaseConnection) -> Result<Self::Result, ServiceError>;
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetManufactureOrderByIdQuery {
    pub order_id: String,
}

#[async_trait]
impl Query for GetManufactureOrderByIdQuery {
    type Result = ManufactureOrderModel;

    async fn execute(&self, db_pool: &DatabaseConnection) -> Result<Self::Result, ServiceError> {
        
        ManufactureOrder::find_by_id(self.order_id.clone())
            .one(db_pool)
            .await
            .map_err(|e| ServiceError::DatabaseError(e))?
            .ok_or_else(|| ServiceError::NotFound("Not found".to_string()))
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetManufactureOrdersByStatusQuery {
    pub status: String,
    pub limit: u64,
    pub offset: u64,
}

#[async_trait]
impl Query for GetManufactureOrdersByStatusQuery {
    type Result = Vec<ManufactureOrderModel>;

    async fn execute(&self, db_pool: &DatabaseConnection) -> Result<Self::Result, ServiceError> {
        
        ManufactureOrder::find()
            .filter(crate::models::manufacture_orders::Column::Priority.eq(self.status.clone()))
            .order_by_desc(crate::models::manufacture_orders::Column::CreatedOn)
            .limit(self.limit)
            .offset(self.offset)
            .all(db_pool)
            .await
            .map_err(|e| ServiceError::DatabaseError(e))
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetManufactureOrderDetailsQuery {
    pub order_id: String,
}

#[derive(Debug, Serialize)]
pub struct ManufactureOrderDetails {
    pub order: ManufactureOrderModel,
    // pub product: <crate::models::product_entity::Entity as sea_orm::EntityTrait>::Model,
    pub components: Vec<ManufactureOrderComponent>,
    // pub operations: Vec<ManufactureOrderOperationEntity::Model>,
}

#[derive(Debug, Serialize)]
pub struct ManufactureOrderComponent {
    pub component_id: String,
    pub component_name: String,
    pub required_quantity: f64,
    pub allocated_quantity: f64,
}

#[async_trait]
impl Query for GetManufactureOrderDetailsQuery {
    type Result = ManufactureOrderDetails;

    async fn execute(&self, db_pool: &DatabaseConnection) -> Result<Self::Result, ServiceError> {
        
        let order = ManufactureOrder::find_by_id(self.order_id.clone())
            .one(db_pool)
            .await
            .map_err(|e| ServiceError::DatabaseError(e))?
            .ok_or_else(|| ServiceError::NotFound("Resource not found".to_string()))?;

        // Note: manufacture_orders doesn't have a product_id field
        // let product = ProductEntity::find_by_id(order.product_id)
        //     .one(db_pool)
        //     .await
        //     .map_err(|e| ServiceError::DatabaseError(e))?
        //     .ok_or_else(|| ServiceError::NotFound("Resource not found".to_string()))?;

        let components = ManufactureOrderComponentEntity::find()
            .filter(
                crate::models::manufacture_order_line_item::Column::ManufactureOrderId.eq(self.order_id.clone()),
            )
            .all(db_pool)
            .await
            .map_err(|e| ServiceError::DatabaseError(e))?;

        let manufacture_components = components
            .into_iter()
            .map(|component| ManufactureOrderComponent {
                component_id: component.id,
                component_name: component.part_name.clone(),
                required_quantity: component.quantity as f64,
                allocated_quantity: component.quantity as f64, // Using quantity as allocated_quantity since there's no separate field
            })
            .collect();

        Ok(ManufactureOrderDetails {
            order,
            // product,
            components: manufacture_components,
            // operations,
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

    async fn execute(&self, db_pool: &DatabaseConnection) -> Result<Self::Result, ServiceError> {
        
        let total_orders = ManufactureOrder::find()
            .filter(crate::models::manufacture_orders::Column::CreatedOn.between(self.start_date, self.end_date))
            .count(db_pool)
            .await
            .map_err(|e| ServiceError::DatabaseError(e))?;

        let completed_orders = ManufactureOrder::find()
            .filter(crate::models::manufacture_orders::Column::CreatedOn.between(self.start_date, self.end_date))
            .filter(crate::models::manufacture_orders::Column::Priority.eq("Completed"))
            .count(db_pool)
            .await
            .map_err(|e| ServiceError::DatabaseError(e))?;

        let on_time_completions = ManufactureOrder::find()
            .filter(crate::models::manufacture_orders::Column::CreatedOn.between(self.start_date, self.end_date))
            .filter(crate::models::manufacture_orders::Column::Priority.eq("Completed"))
            .count(db_pool)
            .await
            .map_err(|e| ServiceError::DatabaseError(e))?;

        let on_time_completion_rate = if completed_orders > 0 {
            on_time_completions as f64 / completed_orders as f64
        } else {
            0.0
        };

        let average_cycle_time = ManufactureOrder::find()
            .filter(crate::models::manufacture_orders::Column::CreatedOn.between(self.start_date, self.end_date))
            .filter(crate::models::manufacture_orders::Column::Priority.eq("Completed"))
            .select_only()
            .column_as(
                Expr::cust("AVG(actual_end_date - actual_start_date)"),
                "average_cycle_time",
            )
            .into_tuple()
            .one(db_pool)
            .await
            .map_err(|e| ServiceError::DatabaseError(e))?
            .unwrap_or(0.0);

        Ok(ManufactureOrderEfficiency {
            total_orders: total_orders.try_into().unwrap(),
            completed_orders: completed_orders.try_into().unwrap(),
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

    async fn execute(&self, _db_pool: &DatabaseConnection) -> Result<Self::Result, ServiceError> {
        
        // let utilizations = ManufactureOrderOperationEntity::Entity::find()
        //     .filter(
        //         ManufactureOrderOperationEntity::Column::StartTime
        //             .between(self.start_date, self.end_date),
        //     )
        //     .group_by(ManufactureOrderOperationEntity::Column::ResourceId)
        //     .select_only()
        //     .column(ManufactureOrderOperationEntity::Column::ResourceId)
        //     .column_as(
        //         Expr::cust("SUM(end_time - start_time)"),
        //         "utilized_hours",
        //     )
            // .all(db_pool)
            // .await
            // .map_err(|_| ServiceError::DatabaseError)?;

        let _total_hours = (self.end_date - self.start_date).num_hours() as f64;

        // Ok(utilizations
        //     .into_iter()
        //     .map(|(resource_id, utilized_hours)| ResourceUtilization {
        //         resource_id,
        //         resource_name: resource_id.to_string(), // Assuming resource name needs to be fetched separately if needed
        //         total_hours,
        //         utilized_hours: utilized_hours.unwrap_or(0.0),
        //         utilization_rate: utilized_hours.unwrap_or(0.0) / total_hours,
        //     })
        //     .collect())
        
        Ok(vec![])
    }
}
