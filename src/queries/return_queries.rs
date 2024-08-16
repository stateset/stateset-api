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
use crate::return_entity::Return;
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
pub struct GetReturnByIdQuery {
    pub return_id: i32,
}

#[async_trait]
impl Query for GetReturnByIdQuery {
    type Result = Option<Return::Model>;

    async fn execute(&self, db_pool: Arc<DbPool>) -> Result<Self::Result, ServiceError> {
        let db = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;
        Return::find_by_id(self.return_id)
            .one(&db)
            .await
            .map_err(|_| ServiceError::DatabaseError)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetReturnsByOrderQuery {
    pub order_id: i32,
}

#[async_trait]
impl Query for GetReturnsByOrderQuery {
    type Result = Vec<Return::Model>;

    async fn execute(&self, db_pool: Arc<DbPool>) -> Result<Self::Result, ServiceError> {
        let db = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;
        Return::find()
            .filter(Return::Column::OrderId.eq(self.order_id))
            .all(&db)
            .await
            .map_err(|_| ServiceError::DatabaseError)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetReturnsByDateRangeQuery {
    pub start_date: DateTime<Utc>,
    pub end_date: DateTime<Utc>,
    pub limit: u64,
    pub offset: u64,
}

#[async_trait]
impl Query for GetReturnsByDateRangeQuery {
    type Result = Vec<Return::Model>;

    async fn execute(&self, db_pool: Arc<DbPool>) -> Result<Self::Result, ServiceError> {
        let db = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;
        Return::find()
            .filter(Return::Column::CreatedAt.between(self.start_date, self.end_date))
            .order_by_desc(Return::Column::CreatedAt)
            .limit(self.limit)
            .offset(self.offset)
            .all(&db)
            .await
            .map_err(|_| ServiceError::DatabaseError)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetTopReturnReasonsQuery {
    pub start_date: DateTime<Utc>,
    pub end_date: DateTime<Utc>,
    pub limit: u64,
}

#[derive(Debug, Serialize)]
pub struct ReturnReasonSummary {
    pub reason: String,
    pub count: i64,
}

#[async_trait]
impl Query for GetTopReturnReasonsQuery {
    type Result = Vec<ReturnReasonSummary>;

    async fn execute(&self, db_pool: Arc<DbPool>) -> Result<Self::Result, ServiceError> {
        let db = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;
        Return::find()
            .filter(Return::Column::CreatedAt.between(self.start_date, self.end_date))
            .group_by(Return::Column::Reason)
            .select_only()
            .column(Return::Column::Reason)
            .column_as(sea_orm::sea_query::Expr::count("*"), "count")
            .order_by_desc(sea_orm::sea_query::Expr::count("*"))
            .limit(self.limit)
            .into_model::<ReturnReasonSummary>()
            .all(&db)
            .await
            .map_err(|_| ServiceError::DatabaseError)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetReturnRateQuery {
    pub start_date: DateTime<Utc>,
    pub end_date: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct ReturnRate {
    pub total_orders: i64,
    pub total_returns: i64,
    pub return_rate: f64,
}

#[async_trait]
impl Query for GetReturnRateQuery {
    type Result = ReturnRate;

    async fn execute(&self, db_pool: Arc<DbPool>) -> Result<Self::Result, ServiceError> {
        let db = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;
        
        let total_orders = Order::find()
            .filter(Order::Column::CreatedAt.between(self.start_date, self.end_date))
            .count(&db)
            .await
            .map_err(|_| ServiceError::DatabaseError)?;

        let total_returns = Return::find()
            .filter(Return::Column::CreatedAt.between(self.start_date, self.end_date))
            .count(&db)
            .await
            .map_err(|_| ServiceError::DatabaseError)?;

        let return_rate = if total_orders > 0 {
            (total_returns as f64 / total_orders as f64) * 100.0
        } else {
            0.0
        };

        Ok(ReturnRate {
            total_orders,
            total_returns,
            return_rate,
        })
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetProductsWithHighestReturnRateQuery {
    pub start_date: DateTime<Utc>,
    pub end_date: DateTime<Utc>,
    pub limit: u64,
}

#[derive(Debug, Serialize)]
pub struct ProductReturnRate {
    pub product_id: i32,
    pub product_name: String,
    pub total_sold: i64,
    pub total_returned: i64,
    pub return_rate: f64,
}

#[async_trait]
impl Query for GetProductsWithHighestReturnRateQuery {
    type Result = Vec<ProductReturnRate>;

    async fn execute(&self, db_pool: Arc<DbPool>) -> Result<Self::Result, ServiceError> {
        let db = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;
        
        // This query is complex and might require raw SQL or a series of JOINs
        // For demonstration, we'll use a simplified version that may not be as efficient
        let results = Product::find()
            .find_with_related(OrderItem::Entity)
            .filter(OrderItem::Column::CreatedAt.between(self.start_date, self.end_date))
            .all(&db)
            .await
            .map_err(|_| ServiceError::DatabaseError)?;

        let mut product_stats: Vec<ProductReturnRate> = results
            .into_iter()
            .map(|(product, order_items)| {
                let total_sold = order_items.len() as i64;
                let total_returned = order_items
                    .iter()
                    .filter(|item| item.is_returned)
                    .count() as i64;
                let return_rate = if total_sold > 0 {
                    (total_returned as f64 / total_sold as f64) * 100.0
                } else {
                    0.0
                };
                ProductReturnRate {
                    product_id: product.id,
                    product_name: product.name,
                    total_sold,
                    total_returned,
                    return_rate,
                }
            })
            .collect();

        product_stats.sort_by(|a, b| b.return_rate.partial_cmp(&a.return_rate).unwrap());
        product_stats.truncate(self.limit as usize);

        Ok(product_stats)
    }
}