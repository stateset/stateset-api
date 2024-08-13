use async_trait::async_trait;
use serde::{Serialize, Deserialize};
use std::sync::Arc;
use crate::{errors::ServiceError, db::DbPool, models::*};
use chrono::{DateTime, Utc};

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
pub struct GetPendingReturnsQuery {
    pub limit: i64,
    pub offset: i64,
}

#[async_trait]
impl Query for GetPendingReturnsQuery {
    type Result = Vec<Return>;

    async fn execute(&self, db_pool: Arc<DbPool>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;
        let pending_returns = returns::table
            .filter(returns::status.eq(ReturnStatus::Pending))
            .limit(self.limit)
            .offset(self.offset)
            .load::<Return>(&conn)
            .map_err(|_| ServiceError::DatabaseError)?;
        Ok(pending_returns)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetActiveWarrantiesQuery {
    pub customer_id: i32,
}

#[async_trait]
impl Query for GetActiveWarrantiesQuery {
    type Result = Vec<Warranty>;

    async fn execute(&self, db_pool: Arc<DbPool>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;
        let active_warranties = warranties::table
            .filter(warranties::customer_id.eq(self.customer_id))
            .filter(warranties::expiry_date.gt(Utc::now().naive_utc()))
            .load::<Warranty>(&conn)
            .map_err(|_| ServiceError::DatabaseError)?;
        Ok(active_warranties)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetShipmentsByDateRangeQuery {
    pub start_date: DateTime<Utc>,
    pub end_date: DateTime<Utc>,
}

#[async_trait]
impl Query for GetShipmentsByDateRangeQuery {
    type Result = Vec<Shipment>;

    async fn execute(&self, db_pool: Arc<DbPool>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;
        let shipments = shipments::table
            .filter(shipments::created_at.between(self.start_date, self.end_date))
            .load::<Shipment>(&conn)
            .map_err(|_| ServiceError::DatabaseError)?;
        Ok(shipments)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetOpenWorkOrdersQuery {
    pub limit: i64,
    pub offset: i64,
}

#[async_trait]
impl Query for GetOpenWorkOrdersQuery {
    type Result = Vec<WorkOrder>;

    async fn execute(&self, db_pool: Arc<DbPool>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;
        let open_work_orders = work_orders::table
            .filter(work_orders::status.eq(WorkOrderStatus::Open))
            .limit(self.limit)
            .offset(self.offset)
            .load::<WorkOrder>(&conn)
            .map_err(|_| ServiceError::DatabaseError)?;
        Ok(open_work_orders)
    }
}