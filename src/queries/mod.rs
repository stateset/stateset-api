use async_trait::async_trait;
use serde::{Serialize, Deserialize};
use std::sync::Arc;
use crate::{errors::ServiceError, db::DbPool};

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

// Example of another query implementation
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
