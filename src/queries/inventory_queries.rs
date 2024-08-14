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
pub struct GetInventoryItemQuery {
    pub product_id: i32,
}

#[async_trait]
impl Query for GetInventoryItemQuery {
    type Result = InventoryItem;

    async fn execute(&self, db_pool: Arc<DbPool>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;
        let item = inventory_items::table
            .filter(inventory_items::product_id.eq(self.product_id))
            .first::<InventoryItem>(&conn)
            .map_err(|_| ServiceError::NotFound)?;
        Ok(item)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetLowStockItemsQuery {
    pub threshold: i32,
    pub limit: i64,
    pub offset: i64,
}

#[async_trait]
impl Query for GetLowStockItemsQuery {
    type Result = Vec<(Product, InventoryItem)>;

    async fn execute(&self, db_pool: Arc<DbPool>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;
        let items = products::table
            .inner_join(inventory_items::table)
            .filter(inventory_items::quantity.le(self.threshold))
            .order(inventory_items::quantity.asc())
            .limit(self.limit)
            .offset(self.offset)
            .load::<(Product, InventoryItem)>(&conn)
            .map_err(|_| ServiceError::DatabaseError)?;
        Ok(items)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetInventoryValueQuery {}

#[derive(Debug, Serialize)]
pub struct InventoryValue {
    pub total_value: f64,
    pub total_items: i64,
}

#[async_trait]
impl Query for GetInventoryValueQuery {
    type Result = InventoryValue;

    async fn execute(&self, db_pool: Arc<DbPool>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;
        let result: (Option<f64>, i64) = inventory_items::table
            .select((
                sum(inventory_items::quantity.cast::<f64>() * inventory_items::unit_cost),
                count(inventory_items::id),
            ))
            .first(&conn)
            .map_err(|_| ServiceError::DatabaseError)?;
        
        Ok(InventoryValue {
            total_value: result.0.unwrap_or(0.0),
            total_items: result.1,
        })
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetInventoryMovementsQuery {
    pub product_id: i32,
    pub start_date: DateTime<Utc>,
    pub end_date: DateTime<Utc>,
    pub limit: i64,
    pub offset: i64,
}

#[async_trait]
impl Query for GetInventoryMovementsQuery {
    type Result = Vec<InventoryMovement>;

    async fn execute(&self, db_pool: Arc<DbPool>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;
        let movements = inventory_movements::table
            .filter(inventory_movements::product_id.eq(self.product_id))
            .filter(inventory_movements::timestamp.between(self.start_date, self.end_date))
            .order(inventory_movements::timestamp.desc())
            .limit(self.limit)
            .offset(self.offset)
            .load::<InventoryMovement>(&conn)
            .map_err(|_| ServiceError::DatabaseError)?;
        Ok(movements)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetTopSellingProductsQuery {
    pub start_date: DateTime<Utc>,
    pub end_date: DateTime<Utc>,
    pub limit: i64,
}

#[derive(Debug, Serialize)]
pub struct TopSellingProduct {
    pub product_id: i32,
    pub product_name: String,
    pub quantity_sold: i64,
    pub total_revenue: f64,
}

#[async_trait]
impl Query for GetTopSellingProductsQuery {
    type Result = Vec<TopSellingProduct>;

    async fn execute(&self, db_pool: Arc<DbPool>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;
        let top_selling = order_items::table
            .inner_join(products::table)
            .inner_join(orders::table.on(order_items::order_id.eq(orders::id)))
            .filter(orders::order_date.between(self.start_date, self.end_date))
            .group_by((products::id, products::name))
            .order(sum(order_items::quantity).desc())
            .limit(self.limit)
            .select((
                products::id,
                products::name,
                sum(order_items::quantity),
                sum(order_items::quantity.cast::<f64>() * order_items::unit_price),
            ))
            .load::<(i32, String, Option<i64>, Option<f64>)>(&conn)
            .map_err(|_| ServiceError::DatabaseError)?;

        Ok(top_selling
            .into_iter()
            .map(|(product_id, product_name, quantity_sold, total_revenue)| TopSellingProduct {
                product_id,
                product_name,
                quantity_sold: quantity_sold.unwrap_or(0),
                total_revenue: total_revenue.unwrap_or(0.0),
            })
            .collect())
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetInventoryTurnoverRatioQuery {
    pub start_date: DateTime<Utc>,
    pub end_date: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct InventoryTurnoverRatio {
    pub ratio: f64,
    pub average_inventory_value: f64,
    pub cost_of_goods_sold: f64,
}

#[async_trait]
impl Query for GetInventoryTurnoverRatioQuery {
    type Result = InventoryTurnoverRatio;

    async fn execute(&self, db_pool: Arc<DbPool>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;
        
        // Calculate average inventory value
        let avg_inventory: f64 = inventory_snapshots::table
            .filter(inventory_snapshots::date.between(self.start_date, self.end_date))
            .select(avg(inventory_snapshots::total_value))
            .first::<Option<f64>>(&conn)
            .map_err(|_| ServiceError::DatabaseError)?
            .unwrap_or(0.0);

        // Calculate cost of goods sold
        let cogs: f64 = order_items::table
            .inner_join(orders::table.on(order_items::order_id.eq(orders::id)))
            .filter(orders::order_date.between(self.start_date, self.end_date))
            .select(sum(order_items::quantity.cast::<f64>() * order_items::unit_cost))
            .first::<Option<f64>>(&conn)
            .map_err(|_| ServiceError::DatabaseError)?
            .unwrap_or(0.0);

        let ratio = if avg_inventory > 0.0 { cogs / avg_inventory } else { 0.0 };

        Ok(InventoryTurnoverRatio {
            ratio,
            average_inventory_value: avg_inventory,
            cost_of_goods_sold: cogs,
        })
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetInventoryForecastQuery {
    pub product_id: i32,
    pub forecast_period: i32, // in days
}

#[derive(Debug, Serialize)]
pub struct InventoryForecast {
    pub product_id: i32,
    pub current_stock: i32,
    pub forecasted_demand: i32,
    pub recommended_reorder: i32,
}

#[async_trait]
impl Query for GetInventoryForecastQuery {
    type Result = InventoryForecast;

    async fn execute(&self, db_pool: Arc<DbPool>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;
        
        // Get current stock
        let current_stock = inventory_items::table
            .filter(inventory_items::product_id.eq(self.product_id))
            .select(inventory_items::quantity)
            .first::<i32>(&conn)
            .map_err(|_| ServiceError::NotFound)?;

        // Calculate average daily demand for the last 30 days
        let end_date = Utc::now();
        let start_date = end_date - chrono::Duration::days(30);
        let avg_daily_demand: f64 = order_items::table
            .inner_join(orders::table.on(order_items::order_id.eq(orders::id)))
            .filter(order_items::product_id.eq(self.product_id))
            .filter(orders::order_date.between(start_date, end_date))
            .select(sum(order_items::quantity).cast::<f64>() / 30.0)
            .first::<Option<f64>>(&conn)
            .map_err(|_| ServiceError::DatabaseError)?
            .unwrap_or(0.0);

        let forecasted_demand = (avg_daily_demand * self.forecast_period as f64) as i32;
        let recommended_reorder = if forecasted_demand > current_stock {
            forecasted_demand - current_stock
        } else {
            0
        };

        Ok(InventoryForecast {
            product_id: self.product_id,
            current_stock,
            forecasted_demand,
            recommended_reorder,
        })
    }
}