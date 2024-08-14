use async_trait::async_trait;
use serde::{Serialize, Deserialize};
use std::sync::Arc;
use crate::{errors::ServiceError, db::DbPool, models::*};
use chrono::{DateTime, Utc};
use diesel::prelude::*;
use diesel::dsl::*;
use crate::queries::shipments::GetShipmentsByDateRangeQuery;

#[async_trait]
pub trait Query: Send + Sync {
    type Result: Send + Sync;

    async fn execute(&self, db_pool: Arc<DbPool>) -> Result<Self::Result, ServiceError>;
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetOrderQuery {
    pub order_id: i32,
}

#[async_trait]
impl Query for GetOrderQuery {
    type Result = Order;

    async fn execute(&self, db_pool: Arc<DbPool>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;
        let order = orders::table
            .find(self.order_id)
            .first::<Order>(&conn)
            .map_err(|_| ServiceError::NotFound)?;
        Ok(order)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetCustomerOrdersQuery {
    pub customer_id: i32,
}

#[async_trait]
impl Query for GetCustomerOrdersQuery {
    type Result = Vec<Order>;

    async fn execute(&self, db_pool: Arc<DbPool>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;
        let orders = orders::table
            .filter(orders::customer_id.eq(self.customer_id))
            .load::<Order>(&conn)
            .map_err(|_| ServiceError::NotFound)?;
        Ok(orders)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetOrdersByStatusQuery {
    pub status: OrderStatus,
    pub limit: i64,
    pub offset: i64,
}

#[async_trait]
impl Query for GetOrdersByStatusQuery {
    type Result = Vec<Order>;

    async fn execute(&self, db_pool: Arc<DbPool>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;
        let orders = orders::table
            .filter(orders::status.eq(self.status))
            .limit(self.limit)
            .offset(self.offset)
            .load::<Order>(&conn)
            .map_err(|_| ServiceError::DatabaseError)?;
        Ok(orders)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetProductInventoryQuery {
    pub product_id: i32,
}

#[async_trait]
impl Query for GetProductInventoryQuery {
    type Result = Inventory;

    async fn execute(&self, db_pool: Arc<DbPool>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;
        let inventory = inventory::table
            .find(self.product_id)
            .first::<Inventory>(&conn)
            .map_err(|_| ServiceError::NotFound)?;
        Ok(inventory)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetLowStockProductsQuery {
    pub threshold: i32,
}

#[async_trait]
impl Query for GetLowStockProductsQuery {
    type Result = Vec<(Product, Inventory)>;

    async fn execute(&self, db_pool: Arc<DbPool>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;
        let low_stock = products::table
            .inner_join(inventory::table)
            .filter(inventory::quantity.lt(self.threshold))
            .load::<(Product, Inventory)>(&conn)
            .map_err(|_| ServiceError::DatabaseError)?;
        Ok(low_stock)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetOrdersInDateRangeQuery {
    pub start_date: DateTime<Utc>,
    pub end_date: DateTime<Utc>,
    pub limit: i64,
    pub offset: i64,
}

#[async_trait]
impl Query for GetOrdersInDateRangeQuery {
    type Result = Vec<Order>;

    async fn execute(&self, db_pool: Arc<DbPool>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;
        let orders = orders::table
            .filter(orders::created_at.between(self.start_date, self.end_date))
            .order(orders::created_at.desc())
            .limit(self.limit)
            .offset(self.offset)
            .load::<Order>(&conn)
            .map_err(|_| ServiceError::DatabaseError)?;
        Ok(orders)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetTopSellingProductsQuery {
    pub start_date: DateTime<Utc>,
    pub end_date: DateTime<Utc>,
    pub limit: i64,
}

#[async_trait]
impl Query for GetTopSellingProductsQuery {
    type Result = Vec<(Product, i64)>; // (Product, TotalQuantitySold)

    async fn execute(&self, db_pool: Arc<DbPool>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;
        let top_selling = order_items::table
            .inner_join(orders::table)
            .inner_join(products::table)
            .filter(orders::created_at.between(self.start_date, self.end_date))
            .group_by(products::id)
            .order(sum(order_items::quantity).desc())
            .limit(self.limit)
            .select((products::all_columns, sum(order_items::quantity)))
            .load::<(Product, i64)>(&conn)
            .map_err(|_| ServiceError::DatabaseError)?;
        Ok(top_selling)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetOrderDetailsQuery {
    pub order_id: i32,
}

#[derive(Debug, Serialize)]
pub struct OrderDetails {
    pub order: Order,
    pub customer: Customer,
    pub items: Vec<(OrderItem, Product)>,
}

#[async_trait]
impl Query for GetOrderDetailsQuery {
    type Result = OrderDetails;

    async fn execute(&self, db_pool: Arc<DbPool>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;
        
        let order = orders::table
            .find(self.order_id)
            .first::<Order>(&conn)
            .map_err(|_| ServiceError::NotFound)?;

        let customer = customers::table
            .find(order.customer_id)
            .first::<Customer>(&conn)
            .map_err(|_| ServiceError::NotFound)?;

        let items = OrderItem::belonging_to(&order)
            .inner_join(products::table)
            .load::<(OrderItem, Product)>(&conn)
            .map_err(|_| ServiceError::DatabaseError)?;

        Ok(OrderDetails {
            order,
            customer,
            items,
        })
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetOrderStatusSummaryQuery {
    pub start_date: DateTime<Utc>,
    pub end_date: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct OrderStatusSummary {
    pub status: OrderStatus,
    pub count: i64,
}

#[async_trait]
impl Query for GetOrderStatusSummaryQuery {
    type Result = Vec<OrderStatusSummary>;

    async fn execute(&self, db_pool: Arc<DbPool>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;
        let summary = orders::table
            .filter(orders::created_at.between(self.start_date, self.end_date))
            .group_by(orders::status)
            .select((orders::status, count(orders::id)))
            .load::<(OrderStatus, i64)>(&conn)
            .map_err(|_| ServiceError::DatabaseError)?;

        Ok(summary
            .into_iter()
            .map(|(status, count)| OrderStatusSummary { status, count })
            .collect())
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetAverageOrderValueQuery {
    pub start_date: DateTime<Utc>,
    pub end_date: DateTime<Utc>,
}

#[async_trait]
impl Query for GetAverageOrderValueQuery {
    type Result = f64;

    async fn execute(&self, db_pool: Arc<DbPool>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;
        let average_value: Option<f64> = orders::table
            .filter(orders::created_at.between(self.start_date, self.end_date))
            .select(avg(orders::total_amount))
            .first(&conn)
            .map_err(|_| ServiceError::DatabaseError)?;

        Ok(average_value.unwrap_or(0.0))
    }
}