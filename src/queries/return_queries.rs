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
pub struct GetReturnByIdQuery {
    pub return_id: i32,
}

#[async_trait]
impl Query for GetReturnByIdQuery {
    type Result = Return;

    async fn execute(&self, db_pool: Arc<DbPool>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;
        let return_record = returns::table
            .find(self.return_id)
            .first::<Return>(&conn)
            .map_err(|_| ServiceError::NotFound)?;
        Ok(return_record)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetReturnsByOrderQuery {
    pub order_id: i32,
}

#[async_trait]
impl Query for GetReturnsByOrderQuery {
    type Result = Vec<Return>;

    async fn execute(&self, db_pool: Arc<DbPool>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;
        let returns = returns::table
            .filter(returns::order_id.eq(self.order_id))
            .load::<Return>(&conn)
            .map_err(|_| ServiceError::DatabaseError)?;
        Ok(returns)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetReturnsByDateRangeQuery {
    pub start_date: DateTime<Utc>,
    pub end_date: DateTime<Utc>,
    pub limit: i64,
    pub offset: i64,
}

#[async_trait]
impl Query for GetReturnsByDateRangeQuery {
    type Result = Vec<Return>;

    async fn execute(&self, db_pool: Arc<DbPool>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;
        let returns = returns::table
            .filter(returns::created_at.between(self.start_date, self.end_date))
            .order(returns::created_at.desc())
            .limit(self.limit)
            .offset(self.offset)
            .load::<Return>(&conn)
            .map_err(|_| ServiceError::DatabaseError)?;
        Ok(returns)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetTopReturnReasonsQuery {
    pub start_date: DateTime<Utc>,
    pub end_date: DateTime<Utc>,
    pub limit: i64,
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
        let conn = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;
        let top_reasons = returns::table
            .filter(returns::created_at.between(self.start_date, self.end_date))
            .group_by(returns::reason)
            .order(count_star().desc())
            .limit(self.limit)
            .select((returns::reason, count_star()))
            .load::<(String, i64)>(&conn)
            .map_err(|_| ServiceError::DatabaseError)?;

        Ok(top_reasons
            .into_iter()
            .map(|(reason, count)| ReturnReasonSummary { reason, count })
            .collect())
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
        let conn = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;
        
        let total_orders: i64 = orders::table
            .filter(orders::created_at.between(self.start_date, self.end_date))
            .count()
            .get_result(&conn)
            .map_err(|_| ServiceError::DatabaseError)?;

        let total_returns: i64 = returns::table
            .filter(returns::created_at.between(self.start_date, self.end_date))
            .count()
            .get_result(&conn)
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
    pub limit: i64,
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
        let conn = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;
        
        // This query is more complex and might require raw SQL or a series of queries
        // For demonstration, we'll use a simplified version
        let results = products::table
            .left_join(order_items::table.on(products::id.eq(order_items::product_id)))
            .left_join(returns::table.on(order_items::id.eq(returns::order_item_id)))
            .filter(order_items::created_at.between(self.start_date, self.end_date))
            .group_by(products::id)
            .select((
                products::id,
                products::name,
                count(order_items::id),
                count(returns::id),
            ))
            .order((count(returns::id).desc(), count(order_items::id).desc()))
            .limit(self.limit)
            .load::<(i32, String, i64, i64)>(&conn)
            .map_err(|_| ServiceError::DatabaseError)?;

        Ok(results
            .into_iter()
            .map(|(product_id, product_name, total_sold, total_returned)| {
                let return_rate = if total_sold > 0 {
                    (total_returned as f64 / total_sold as f64) * 100.0
                } else {
                    0.0
                };
                ProductReturnRate {
                    product_id,
                    product_name,
                    total_sold,
                    total_returned,
                    return_rate,
                }
            })
            .collect())
    }
}